//! Application runtime, event loop, and keyboard handling.

pub mod browser;
pub mod editor;
pub mod events;
pub mod state;

use crate::app::events::{
    MutationRequest, WorkerMessage, spawn_apply_mutation, spawn_load_pull_request_data,
    spawn_load_pull_request_diff, spawn_load_pull_requests, spawn_load_specific_pull_request,
};
use crate::app::state::{AppState, ReviewSubmissionEvent, ReviewTab};
use crate::domain::{CommentRef, PullRequestSummary, Route};
use crate::github::client::create_client;
use crate::render::markdown::MarkdownRenderer;
use crate::ui;
use anyhow::Context;
use browser::open_in_browser;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

type WorkerTx = UnboundedSender<WorkerMessage>;

/// Runtime configuration provided by CLI flags.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub pull: Option<u64>,
    pub syntax_theme: String,
}

struct DataContext {
    client: octocrab::Octocrab,
    owner: Option<String>,
    repo: Option<String>,
}

/// Runs the interactive TUI application.
pub async fn run(config: AppConfig) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<WorkerMessage>();

    let mut state = AppState::default();

    let client = create_client()
        .await
        .context("failed to create authenticated GitHub client")?;

    if let Some(pull_number) = config.pull {
        state.begin_operation(format!("Loading pull request #{pull_number}"));
        spawn_load_specific_pull_request(
            tx.clone(),
            client.clone(),
            config.owner.clone(),
            config.repo.clone(),
            pull_number,
        );
    } else {
        state.begin_operation("Loading open pull requests");
        spawn_load_pull_requests(
            tx.clone(),
            client.clone(),
            config.owner.clone(),
            config.repo.clone(),
        );
    }

    let context = DataContext {
        client,
        owner: config.owner,
        repo: config.repo,
    };

    let mut terminal = setup_terminal()?;
    let mut markdown = MarkdownRenderer::new();
    let configured_theme = config.syntax_theme.trim();
    if !configured_theme.is_empty() && !markdown.set_syntax_theme(configured_theme) {
        state.error_message = Some(format!(
            "unknown syntax theme `{configured_theme}`; using `{}`",
            markdown.current_syntax_theme_name()
        ));
    }

    let result = run_event_loop(
        &mut terminal,
        &mut state,
        &context,
        &tx,
        &mut rx,
        &mut markdown,
    )
    .await;

    restore_terminal(&mut terminal)?;
    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    rx: &mut UnboundedReceiver<WorkerMessage>,
    markdown: &mut MarkdownRenderer,
) -> anyhow::Result<()> {
    loop {
        state.advance_spinner();

        while let Ok(message) = rx.try_recv() {
            process_worker_message(state, message, markdown, context, tx);
            maybe_spawn_search_load(state, context, tx);
        }

        terminal.draw(|frame| ui::render(frame, state, markdown))?;

        if state.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(60))? {
            match event::read()? {
                Event::Key(key_event) => {
                    if key_event.kind == KeyEventKind::Press {
                        handle_key_event(terminal, state, context, tx, key_event, markdown);
                    }
                }
                Event::Mouse(mouse_event) => {
                    handle_mouse_event(state, mouse_event);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn process_worker_message(
    state: &mut AppState,
    message: WorkerMessage,
    markdown: &mut MarkdownRenderer,
    context: &DataContext,
    tx: &WorkerTx,
) {
    match message {
        WorkerMessage::PullRequestsLoaded {
            repository_label,
            result,
        } => {
            if state.route == Route::Search {
                state.end_operation();
            }
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
                    markdown.clear_diff_cache();

                    if let Some(review) = state.review.as_mut()
                        && review.pull.number == pull.number
                        && review.pull.owner == pull.owner
                        && review.pull.repo == pull.repo
                    {
                        review.clear_diff();
                        review.set_data(data);
                        state.route = Route::Review;
                        return;
                    }

                    state.open_review(pull, data);
                }
                Err(error) => {
                    state.error_message = Some(error);
                }
            }
        }
        WorkerMessage::PullRequestResolved {
            repository_label,
            pull_number,
            result,
        } => {
            state.end_operation();
            state.set_repository_label(repository_label);

            match result {
                Ok(pull) => {
                    state.error_message = None;
                    state.begin_operation(format!("Loading pull request #{pull_number}"));
                    spawn_load_pull_request_data(tx.clone(), context.client.clone(), pull);
                }
                Err(error) => {
                    state.error_message = Some(error);
                    state.route = Route::Search;
                }
            }
        }
        WorkerMessage::PullRequestDiffLoaded { pull, result } => {
            state.end_operation();
            markdown.clear_diff_cache();

            let Some(review) = state.review.as_mut() else {
                return;
            };
            if review.pull.number != pull.number
                || review.pull.owner != pull.owner
                || review.pull.repo != pull.repo
            {
                return;
            }

            match result {
                Ok(diff) => {
                    state.error_message = None;
                    review.set_diff(diff);
                }
                Err(error) => {
                    review.set_diff_error(error);
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
                    markdown.clear_diff_cache();
                    if let Some(review) = state.review.as_mut()
                        && review.pull.number == pull.number
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
                Err(error) => {
                    state.error_message = Some(error);
                }
            }
        }
    }
}

fn handle_key_event(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    key: KeyEvent,
    markdown: &mut MarkdownRenderer,
) {
    match state.route {
        Route::Search => handle_search_key_event(state, context, tx, key),
        Route::Review => handle_review_key_event(terminal, state, context, tx, key, markdown),
    }
}

fn handle_mouse_event(state: &mut AppState, mouse: MouseEvent) {
    let Some(delta) = mouse_scroll_delta(mouse.kind) else {
        return;
    };

    match state.route {
        Route::Search => match delta {
            MouseScrollDelta::Up(lines) => {
                for _ in 0..lines {
                    state.search_move_up();
                }
            }
            MouseScrollDelta::Down(lines) => {
                for _ in 0..lines {
                    state.search_move_down();
                }
            }
        },
        Route::Review => {
            let Some(review) = state.review.as_mut() else {
                return;
            };

            match delta {
                MouseScrollDelta::Up(lines) => {
                    for _ in 0..lines {
                        review.scroll_preview_up();
                    }
                }
                MouseScrollDelta::Down(lines) => {
                    for _ in 0..lines {
                        review.scroll_preview_down();
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MouseScrollDelta {
    Up(u16),
    Down(u16),
}

fn mouse_scroll_delta(kind: MouseEventKind) -> Option<MouseScrollDelta> {
    const STEP: u16 = 3;

    match kind {
        MouseEventKind::ScrollUp => Some(MouseScrollDelta::Up(STEP)),
        MouseEventKind::ScrollDown => Some(MouseScrollDelta::Down(STEP)),
        _ => None,
    }
}

fn handle_search_key_event(
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    key: KeyEvent,
) {
    if state.is_search_focused() {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => state.unfocus_search(),
            KeyCode::Backspace => state.search_backspace(),
            KeyCode::Char(ch) => {
                if !ch.is_control() {
                    state.search_push_char(ch);
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
        }
        KeyCode::Char('W') => {
            open_selected_pull_in_browser(state);
        }
        KeyCode::Char('s') => state.focus_search(),
        KeyCode::Down | KeyCode::Char('j') => state.search_move_down(),
        KeyCode::Up | KeyCode::Char('k') => state.search_move_up(),
        KeyCode::Enter => {
            if state.is_busy() {
                return;
            }

            let Some(pull) = state.selected_search_pull().cloned() else {
                return;
            };

            state.error_message = None;
            state.begin_operation(format!("Loading pull request #{}", pull.number));

            spawn_load_pull_request_data(tx.clone(), context.client.clone(), pull);
        }
        KeyCode::Char('R') => {
            if state.is_busy() {
                return;
            }

            state.error_message = None;
            state.begin_operation("Refreshing open pull requests");
            spawn_load_pull_requests(
                tx.clone(),
                context.client.clone(),
                context.owner.clone(),
                context.repo.clone(),
            );
        }
        _ => {}
    }
}

fn handle_review_key_event(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    key: KeyEvent,
    markdown: &mut MarkdownRenderer,
) {
    let active_tab = state
        .review
        .as_ref()
        .map(|review| review.active_tab())
        .unwrap_or(ReviewTab::Threads);

    if active_tab == ReviewTab::Diff
        && state
            .review
            .as_ref()
            .is_some_and(|review| review.is_diff_search_focused())
    {
        if let Some(review) = state.review.as_mut() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => review.unfocus_diff_search(),
                KeyCode::Backspace => review.diff_search_backspace(),
                KeyCode::Char(ch) => {
                    if !ch.is_control() {
                        review.diff_search_push_char(ch);
                    }
                }
                _ => {}
            }
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            state.should_quit = true;
        }
        KeyCode::Char('b') | KeyCode::Esc => {
            state.back_to_search();
            maybe_spawn_search_load(state, context, tx);
        }
        KeyCode::Tab => {
            if let Some(review) = state.review.as_mut() {
                review.next_tab();
            }
            load_active_diff_if_needed(state, tx);
        }
        KeyCode::BackTab => {
            if let Some(review) = state.review.as_mut() {
                review.prev_tab();
            }
            load_active_diff_if_needed(state, tx);
        }
        KeyCode::Char('W') => {
            if active_tab == ReviewTab::Threads {
                open_selected_comment_in_browser(state);
            }
        }
        KeyCode::Char('T') => {
            if active_tab == ReviewTab::Diff {
                markdown.cycle_syntax_theme();
            }
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
                if active_tab == ReviewTab::Threads {
                    review.toggle_selected_thread_collapsed();
                } else if active_tab == ReviewTab::Diff {
                    review.toggle_selected_diff_directory_collapsed();
                }
            }
        }
        KeyCode::Char('n') | KeyCode::Char(']') => {
            if active_tab == ReviewTab::Diff
                && let Some(review) = state.review.as_mut()
            {
                review.jump_next_hunk();
            }
        }
        KeyCode::Char('N') | KeyCode::Char('[') => {
            if active_tab == ReviewTab::Diff
                && let Some(review) = state.review.as_mut()
            {
                review.jump_prev_hunk();
            }
        }
        KeyCode::Char('f') => {
            if active_tab == ReviewTab::Threads
                && let Some(review) = state.review.as_mut()
            {
                review.toggle_resolved_filter();
            }
        }
        KeyCode::Char('R') => {
            if state.is_busy() {
                return;
            }

            if active_tab == ReviewTab::Diff {
                if let Some(review) = state.review.as_mut() {
                    review.clear_diff();
                }
                load_active_diff_if_needed(state, tx);
                return;
            }

            let Some(pull) = state.review.as_ref().map(|review| review.pull.clone()) else {
                return;
            };
            state.error_message = None;
            state.begin_operation(format!("Refreshing pull request #{}", pull.number));
            spawn_load_pull_request_data(tx.clone(), context.client.clone(), pull);
        }
        KeyCode::Char('t') => {
            if active_tab != ReviewTab::Threads {
                return;
            }
            if state.is_busy() {
                return;
            }

            let Some(review) = state.review.as_ref() else {
                return;
            };
            let Some(thread_context) = review.selected_thread_context() else {
                state.error_message = Some("select a review thread row".to_owned());
                return;
            };

            let Some(thread_id) = thread_context.thread_id.clone() else {
                state.error_message =
                    Some("thread cannot be resolved (missing thread id)".to_owned());
                return;
            };

            let resolved = !thread_context.is_resolved;
            let operation = if resolved {
                "Resolving thread"
            } else {
                "Reopening thread"
            };

            execute_mutation(
                state,
                context,
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
            if active_tab == ReviewTab::Threads {
                open_reply_editor(terminal, state);
            }
        }
        KeyCode::Char('x') => {
            if active_tab == ReviewTab::Threads
                && let Some(review) = state.review.as_mut()
                && let Some(context) = review.selected_thread_context()
            {
                review.clear_reply_draft(&context.root_key);
            }
        }
        KeyCode::Char('s') => match active_tab {
            ReviewTab::Threads => send_selected_reply(state, context, tx),
            ReviewTab::Diff => {
                if let Some(review) = state.review.as_mut() {
                    review.focus_diff_search();
                }
            }
        },
        KeyCode::Char('C') => {
            if active_tab == ReviewTab::Threads {
                open_submit_review_editor_and_submit(
                    terminal,
                    state,
                    context,
                    tx,
                    ReviewSubmissionEvent::Comment,
                );
            }
        }
        KeyCode::Char('A') => {
            if active_tab == ReviewTab::Threads {
                open_submit_review_editor_and_submit(
                    terminal,
                    state,
                    context,
                    tx,
                    ReviewSubmissionEvent::Approve,
                );
            }
        }
        KeyCode::Char('X') => {
            if active_tab == ReviewTab::Threads {
                open_submit_review_editor_and_submit(
                    terminal,
                    state,
                    context,
                    tx,
                    ReviewSubmissionEvent::RequestChanges,
                );
            }
        }
        _ => {}
    }
}

fn maybe_spawn_search_load(state: &mut AppState, context: &DataContext, tx: &WorkerTx) {
    if state.is_busy() || !state.pull_requests.is_empty() {
        return;
    }

    if state.route == Route::Search {
        state.begin_operation("Loading open pull requests");
        spawn_load_pull_requests(
            tx.clone(),
            context.client.clone(),
            context.owner.clone(),
            context.repo.clone(),
        );
    }
}

fn load_active_diff_if_needed(state: &mut AppState, tx: &WorkerTx) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };
    if review.active_tab() != ReviewTab::Diff {
        return;
    }
    if review.diff.is_some() || review.diff_error.is_some() {
        return;
    }

    let pull = review.pull.clone();
    let changed_files = review.data.changed_files.clone();

    state.error_message = None;
    state.begin_operation(format!("Loading diff for pull request #{}", pull.number));
    spawn_load_pull_request_diff(tx.clone(), pull, changed_files);
}

fn open_reply_editor(terminal: &mut Terminal<CrosstermBackend<Stdout>>, state: &mut AppState) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_mut() else {
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

    match editor::edit_with_system_editor(&existing, terminal) {
        Ok(Some(edited)) => {
            review.set_reply_draft(context.root_key, edited);
            state.error_message = None;
        }
        Ok(None) => {}
        Err(err) => {
            state.error_message = Some(format!("failed to open editor: {err}"));
        }
    }
}

fn send_selected_reply(state: &mut AppState, context: &DataContext, tx: &WorkerTx) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };
    let Some(thread_context) = review.selected_thread_context() else {
        return;
    };

    let body = review
        .reply_drafts
        .get(&thread_context.root_key)
        .map(|value| value.trim().to_owned())
        .unwrap_or_default();

    if body.is_empty() {
        state.error_message = Some("reply is empty; press [e] to edit".to_owned());
        return;
    }

    execute_mutation(
        state,
        context,
        tx,
        review.pull.clone(),
        MutationRequest::ReplyToReviewComment {
            owner: review.pull.owner.clone(),
            repo: review.pull.repo.clone(),
            pull_number: review.pull.number,
            comment_id: thread_context.comment_id,
            body,
        },
        Some(thread_context.root_key),
        "Posting reply",
    );
}

fn open_submit_review_editor_and_submit(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    event: ReviewSubmissionEvent,
) {
    if state.is_busy() {
        return;
    }

    let Some(review) = state.review.as_ref() else {
        return;
    };

    let pull = review.pull.clone();
    let initial = String::new();

    let body = match editor::edit_with_system_editor(&initial, terminal) {
        Ok(Some(text)) => text.trim().to_owned(),
        Ok(None) => return,
        Err(err) => {
            state.error_message = Some(format!("failed to open editor: {err}"));
            return;
        }
    };

    execute_mutation(
        state,
        context,
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

fn execute_mutation(
    state: &mut AppState,
    context: &DataContext,
    tx: &WorkerTx,
    pull: PullRequestSummary,
    mutation: MutationRequest,
    clear_reply_root_key: Option<String>,
    operation_label: impl Into<String>,
) {
    let operation_label = operation_label.into();
    state.error_message = None;

    state.begin_operation(operation_label);
    spawn_apply_mutation(
        tx.clone(),
        context.client.clone(),
        pull,
        mutation,
        clear_reply_root_key,
    );
}

fn open_selected_pull_in_browser(state: &mut AppState) {
    let Some(pull) = state.selected_search_pull() else {
        return;
    };

    let Some(url) = pull
        .html_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    else {
        state.error_message = Some("selected pull request has no web URL".to_owned());
        return;
    };

    match open_in_browser(url) {
        Ok(()) => state.error_message = None,
        Err(err) => {
            state.error_message = Some(format!("failed to open browser: {err}"));
        }
    }
}

fn open_selected_comment_in_browser(state: &mut AppState) {
    let Some(review) = state.review.as_ref() else {
        return;
    };
    let Some(node) = review.selected_node() else {
        return;
    };

    let url = match &node.comment {
        CommentRef::Review(comment) => comment.html_url.as_str(),
        CommentRef::Issue(comment) => comment.html_url.as_str(),
        CommentRef::ReviewSummary(review) => review.html_url.as_str(),
    };

    if url.trim().is_empty() {
        state.error_message = Some("selected comment has no web URL".to_owned());
        return;
    }

    match open_in_browser(url) {
        Ok(()) => state.error_message = None,
        Err(err) => {
            state.error_message = Some(format!("failed to open browser: {err}"));
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
