//! Application runtime, event loop, and keyboard handling.

pub mod events;
pub mod state;

use crate::app::events::{
    MutationRequest, WorkerMessage, spawn_apply_mutation, spawn_load_pull_request_data,
    spawn_load_pull_requests,
};
use crate::app::state::{AppState, InputAction, InputState, ReviewSubmissionEvent};
use crate::domain::Route;
#[cfg(feature = "harness")]
use crate::fixtures;
use crate::github::client::create_client;
use crate::render::markdown::MarkdownRenderer;
use crate::ui;
use anyhow::Context;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver};

/// Runtime configuration provided by CLI flags.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub owner: Option<String>,
    pub repo: Option<String>,
    #[cfg(feature = "harness")]
    pub demo: bool,
}

enum DataMode {
    #[cfg(feature = "harness")]
    Demo,
    Live {
        client: octocrab::Octocrab,
        owner: Option<String>,
        repo: Option<String>,
    },
}

/// Runs the interactive TUI application.
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<WorkerMessage>();

    let mut state = AppState::default();
    let mode = {
        #[cfg(feature = "harness")]
        {
            if config.demo {
                initialize_demo_state(&mut state);
                DataMode::Demo
            } else {
                state.begin_operation("Loading open pull requests");

                let client = create_client()
                    .await
                    .context("failed to create authenticated GitHub client")?;

                spawn_load_pull_requests(
                    tx.clone(),
                    client.clone(),
                    config.owner.clone(),
                    config.repo.clone(),
                );

                DataMode::Live {
                    client,
                    owner: config.owner,
                    repo: config.repo,
                }
            }
        }
        #[cfg(not(feature = "harness"))]
        {
            state.begin_operation("Loading open pull requests");

            let client = create_client()
                .await
                .context("failed to create authenticated GitHub client")?;

            spawn_load_pull_requests(
                tx.clone(),
                client.clone(),
                config.owner.clone(),
                config.repo.clone(),
            );

            DataMode::Live {
                client,
                owner: config.owner,
                repo: config.repo,
            }
        }
    };

    let mut terminal = setup_terminal()?;
    let mut markdown = MarkdownRenderer::new();

    let result = run_event_loop(
        &mut terminal,
        &mut state,
        &mode,
        &tx,
        &mut rx,
        &mut markdown,
    )
    .await;

    restore_terminal(&mut terminal)?;
    result
}

#[cfg(feature = "harness")]
fn initialize_demo_state(state: &mut AppState) {
    let pulls = fixtures::demo_pull_requests();
    let label = pulls
        .first()
        .map(|pull| format!("{}/{}", pull.owner, pull.repo))
        .unwrap_or_else(|| "demo/repository".to_owned());

    state.set_repository_label(label);
    state.set_pull_requests(pulls);
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    rx: &mut UnboundedReceiver<WorkerMessage>,
    markdown: &mut MarkdownRenderer,
) -> anyhow::Result<()> {
    loop {
        state.advance_spinner();

        while let Ok(message) = rx.try_recv() {
            process_worker_message(state, message);
        }

        terminal.draw(|frame| ui::render(frame, state, markdown))?;

        if state.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(60))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Press {
                    handle_key_event(state, mode, tx, key_event);
                }
            }
        }
    }

    Ok(())
}

fn process_worker_message(state: &mut AppState, message: WorkerMessage) {
    match message {
        WorkerMessage::PullRequestsLoaded {
            repository_label,
            result,
        } => {
            state.end_operation();
            state.set_repository_label(repository_label);

            match result {
                Ok(pulls) => {
                    state.error_message = None;
                    state.set_pull_requests(pulls);
                }
                Err(error) => {
                    state.error_message = Some(error);
                }
            }
        }
        WorkerMessage::PullRequestDataLoaded { pull, result } => {
            state.end_operation();

            match result {
                Ok(data) => {
                    state.error_message = None;

                    if let Some(review) = state.review.as_mut() {
                        if review.pull.number == pull.number
                            && review.pull.owner == pull.owner
                            && review.pull.repo == pull.repo
                        {
                            review.set_data(data);
                            state.route = Route::Review;
                            return;
                        }
                    }

                    state.open_review(pull, data);
                }
                Err(error) => {
                    state.error_message = Some(error);
                }
            }
        }
        WorkerMessage::MutationApplied {
            pull,
            clear_reply_root_key,
            result,
        } => {
            state.end_operation();

            match result {
                Ok(data) => {
                    state.error_message = None;
                    if let Some(review) = state.review.as_mut() {
                        if review.pull.number == pull.number
                            && review.pull.owner == pull.owner
                            && review.pull.repo == pull.repo
                        {
                            if let Some(root_key) = clear_reply_root_key {
                                review.clear_reply_draft(&root_key);
                            }
                            review.set_data(data);
                            state.route = Route::Review;
                        }
                    }
                }
                Err(error) => {
                    state.error_message = Some(error);
                }
            }
        }
    }
}

fn handle_key_event(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    key: KeyEvent,
) {
    if state.input.is_some() {
        handle_input_key_event(state, mode, tx, key);
        return;
    }

    match state.route {
        Route::Search => handle_search_key_event(state, mode, tx, key),
        Route::Review => handle_review_key_event(state, mode, tx, key),
    }
}

fn handle_input_key_event(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Esc => state.cancel_input(),
        KeyCode::Backspace => {
            if let Some(input) = state.input.as_mut() {
                input.backspace();
            }
        }
        KeyCode::Enter => {
            let Some(input) = state.input.take() else {
                return;
            };

            let body = input.trimmed();
            match input.action {
                InputAction::Reply {
                    root_key,
                    comment_id,
                    pull,
                } => {
                    if body.is_empty() {
                        state.error_message = Some("reply is empty".to_owned());
                        return;
                    }

                    if let Some(review) = state.review.as_mut() {
                        review.set_reply_draft(root_key.clone(), body.clone());
                    }

                    execute_mutation(
                        state,
                        mode,
                        tx,
                        pull.clone(),
                        MutationRequest::ReplyToReviewComment {
                            owner: pull.owner.clone(),
                            repo: pull.repo.clone(),
                            pull_number: pull.number,
                            comment_id,
                            body,
                        },
                        Some(root_key),
                        "Posting reply",
                    );
                }
                InputAction::SubmitReview { event, pull } => {
                    execute_mutation(
                        state,
                        mode,
                        tx,
                        pull.clone(),
                        MutationRequest::SubmitPullRequestReview {
                            owner: pull.owner.clone(),
                            repo: pull.repo.clone(),
                            pull_number: pull.number,
                            event: event.as_api_event().to_owned(),
                            body,
                        },
                        None,
                        event.title(),
                    );
                }
            }
        }
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                if let Some(input) = state.input.as_mut() {
                    input.push_char(ch);
                }
            }
        }
        _ => {}
    }
}

fn handle_search_key_event(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
        }
        KeyCode::Down | KeyCode::Char('j') => state.search_move_down(),
        KeyCode::Up | KeyCode::Char('k') => state.search_move_up(),
        KeyCode::Backspace => state.search_backspace(),
        KeyCode::Enter => {
            if state.is_busy() {
                return;
            }

            let Some(pull) = state.selected_search_pull().cloned() else {
                return;
            };

            state.error_message = None;
            state.begin_operation(format!("Loading pull request #{}", pull.number));

            match mode {
                #[cfg(feature = "harness")]
                DataMode::Demo => {
                    state.end_operation();
                    let data = fixtures::demo_pull_request_data_for(&pull);
                    state.open_review(pull, data);
                }
                DataMode::Live { client, .. } => {
                    spawn_load_pull_request_data(tx.clone(), client.clone(), pull);
                }
            }
        }
        KeyCode::Char('r') => {
            if state.is_busy() {
                return;
            }

            match mode {
                #[cfg(feature = "harness")]
                DataMode::Demo => {
                    initialize_demo_state(state);
                }
                DataMode::Live {
                    client,
                    owner,
                    repo,
                } => {
                    state.error_message = None;
                    state.begin_operation("Refreshing open pull requests");
                    spawn_load_pull_requests(
                        tx.clone(),
                        client.clone(),
                        owner.clone(),
                        repo.clone(),
                    );
                }
            }
        }
        KeyCode::Char(ch) => {
            if !ch.is_control() {
                state.search_push_char(ch);
            }
        }
        _ => {}
    }
}

fn handle_review_key_event(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
        }
        KeyCode::Char('b') | KeyCode::Esc => {
            state.back_to_search();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(review) = state.review.as_mut() {
                review.move_down();
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(review) = state.review.as_mut() {
                review.move_up();
            }
        }
        KeyCode::PageDown => {
            if let Some(review) = state.review.as_mut() {
                for _ in 0..8 {
                    review.scroll_preview_down();
                }
            }
        }
        KeyCode::PageUp => {
            if let Some(review) = state.review.as_mut() {
                for _ in 0..8 {
                    review.scroll_preview_up();
                }
            }
        }
        KeyCode::Char('o') | KeyCode::Char('z') => {
            if let Some(review) = state.review.as_mut() {
                review.toggle_selected_thread_collapsed();
            }
        }
        KeyCode::Char('f') => {
            if let Some(review) = state.review.as_mut() {
                review.toggle_resolved_filter();
            }
        }
        KeyCode::Char('r') => {
            if state.is_busy() {
                return;
            }

            let Some(pull) = state.review.as_ref().map(|review| review.pull.clone()) else {
                return;
            };

            {
                state.error_message = None;
                state.begin_operation(format!("Refreshing pull request #{}", pull.number));
            }

            match mode {
                #[cfg(feature = "harness")]
                DataMode::Demo => {
                    state.end_operation();
                    let updated = fixtures::demo_pull_request_data_for(&pull);
                    if let Some(review_mut) = state.review.as_mut() {
                        review_mut.set_data(updated);
                    }
                }
                DataMode::Live { client, .. } => {
                    spawn_load_pull_request_data(tx.clone(), client.clone(), pull);
                }
            }
        }
        KeyCode::Char('t') => {
            if state.is_busy() {
                return;
            }

            let Some(review) = state.review.as_ref() else {
                return;
            };
            let Some(context) = review.selected_thread_context() else {
                state.error_message = Some("select a review thread row".to_owned());
                return;
            };

            let Some(thread_id) = context.thread_id.clone() else {
                state.error_message =
                    Some("thread cannot be resolved (missing thread id)".to_owned());
                return;
            };

            let resolved = !context.is_resolved;
            let operation = if resolved {
                "Resolving thread"
            } else {
                "Reopening thread"
            };

            execute_mutation(
                state,
                mode,
                tx,
                review.pull.clone(),
                MutationRequest::SetReviewThreadResolved {
                    thread_id,
                    resolved,
                },
                None,
                operation,
            );
        }
        KeyCode::Char('e') => {
            open_reply_editor(state);
        }
        KeyCode::Char('x') => {
            if let Some(review) = state.review.as_mut() {
                if let Some(context) = review.selected_thread_context() {
                    review.clear_reply_draft(&context.root_key);
                }
            }
        }
        KeyCode::Char('s') => {
            send_selected_reply(state, mode, tx);
        }
        KeyCode::Char('C') => {
            open_submit_review_input(state, ReviewSubmissionEvent::Comment);
        }
        KeyCode::Char('A') => {
            open_submit_review_input(state, ReviewSubmissionEvent::Approve);
        }
        KeyCode::Char('X') => {
            open_submit_review_input(state, ReviewSubmissionEvent::RequestChanges);
        }
        _ => {}
    }
}

fn open_reply_editor(state: &mut AppState) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };
    let Some(context) = review.selected_thread_context() else {
        return;
    };

    let existing = review
        .reply_drafts
        .get(&context.root_key)
        .cloned()
        .unwrap_or_default();

    state.begin_input(InputState {
        title: "Reply to Thread".to_owned(),
        prompt: "Reply body".to_owned(),
        buffer: existing,
        action: InputAction::Reply {
            root_key: context.root_key,
            comment_id: context.comment_id,
            pull: review.pull.clone(),
        },
    });
}

fn send_selected_reply(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };
    let Some(context) = review.selected_thread_context() else {
        return;
    };

    let body = review
        .reply_drafts
        .get(&context.root_key)
        .map(|value| value.trim().to_owned())
        .unwrap_or_default();

    if body.is_empty() {
        state.error_message = Some("reply is empty; press [e] to edit".to_owned());
        return;
    }

    execute_mutation(
        state,
        mode,
        tx,
        review.pull.clone(),
        MutationRequest::ReplyToReviewComment {
            owner: review.pull.owner.clone(),
            repo: review.pull.repo.clone(),
            pull_number: review.pull.number,
            comment_id: context.comment_id,
            body,
        },
        Some(context.root_key),
        "Posting reply",
    );
}

fn open_submit_review_input(state: &mut AppState, event: ReviewSubmissionEvent) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };

    state.begin_input(InputState {
        title: event.title().to_owned(),
        prompt: "Message (optional)".to_owned(),
        buffer: String::new(),
        action: InputAction::SubmitReview {
            event,
            pull: review.pull.clone(),
        },
    });
}

fn execute_mutation(
    state: &mut AppState,
    mode: &DataMode,
    tx: &tokio::sync::mpsc::UnboundedSender<WorkerMessage>,
    pull: crate::domain::PullRequestSummary,
    mutation: MutationRequest,
    clear_reply_root_key: Option<String>,
    operation_label: impl Into<String>,
) {
    let operation_label = operation_label.into();
    state.error_message = None;

    match mode {
        #[cfg(feature = "harness")]
        DataMode::Demo => {
            state.error_message = Some("mutations are disabled in --demo mode".to_owned());
        }
        DataMode::Live { client, .. } => {
            state.begin_operation(operation_label);
            spawn_apply_mutation(
                tx.clone(),
                client.clone(),
                pull,
                mutation,
                clear_reply_root_key,
            );
        }
    }
}

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;

    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)
        .context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend).context("failed to create ratatui terminal")?;

    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;

    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("failed to leave alternate screen")?;

    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}
