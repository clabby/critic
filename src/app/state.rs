//! Application state models and route-local behavior.

use crate::domain::{
    CommentRef, ListNode, ListNodeKind, PullRequestComment, PullRequestData, PullRequestDiffData,
    PullRequestDiffFile, PullRequestSummary, ReviewThread, Route, review_comment_is_outdated,
};
use crate::search::fuzzy::rank_pull_requests;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Spinner frames used for active async operations.
pub const SPINNER_FRAMES: [&str; 8] = ["⢎⡰", "⢎⡡", "⢎⡑", "⢎⠱", "⠎⡱", "⢊⡱", "⢌⡱", "⢆⡱"];

/// Top-level mutable application state.
#[derive(Debug)]
pub struct AppState {
    pub route: Route,
    pub should_quit: bool,
    pub error_message: Option<String>,
    pub repository_label: String,
    pub pull_requests: Vec<PullRequestSummary>,
    pub search_focused: bool,
    pub search_query: String,
    pub search_results: Vec<usize>,
    pub search_selected: usize,
    pub review: Option<ReviewScreenState>,
    operation: Option<OperationState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            route: Route::Search,
            should_quit: false,
            error_message: None,
            repository_label: "(resolving repository)".to_owned(),
            pull_requests: Vec::new(),
            search_focused: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            review: None,
            operation: None,
        }
    }
}

impl AppState {
    pub fn set_repository_label(&mut self, label: String) {
        self.repository_label = label;
    }

    pub fn set_pull_requests(&mut self, pulls: Vec<PullRequestSummary>) {
        self.pull_requests = pulls;
        self.recompute_search();
        self.search_selected = 0;
    }

    pub fn recompute_search(&mut self) {
        self.search_results = rank_pull_requests(&self.search_query, &self.pull_requests)
            .into_iter()
            .map(|result| result.index)
            .collect();

        if self.search_selected >= self.search_results.len() {
            self.search_selected = self.search_results.len().saturating_sub(1);
        }
    }

    pub fn selected_search_pull(&self) -> Option<&PullRequestSummary> {
        let index = *self.search_results.get(self.search_selected)?;
        self.pull_requests.get(index)
    }

    pub fn search_push_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.recompute_search();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.recompute_search();
    }

    pub fn focus_search(&mut self) {
        self.search_focused = true;
    }

    pub fn unfocus_search(&mut self) {
        self.search_focused = false;
    }

    pub fn is_search_focused(&self) -> bool {
        self.search_focused
    }

    pub fn search_move_down(&mut self) {
        if self.search_results.is_empty() {
            self.search_selected = 0;
            return;
        }

        self.search_selected = (self.search_selected + 1).min(self.search_results.len() - 1);
    }

    pub fn search_move_up(&mut self) {
        if self.search_results.is_empty() {
            self.search_selected = 0;
            return;
        }

        self.search_selected = self.search_selected.saturating_sub(1);
    }

    pub fn open_review(&mut self, pull: PullRequestSummary, data: PullRequestData) {
        self.review = Some(ReviewScreenState::new(pull, data));
        self.route = Route::Review;
    }

    pub fn back_to_search(&mut self) {
        self.route = Route::Search;
        self.search_focused = false;
    }

    pub fn begin_operation(&mut self, label: impl Into<String>) {
        self.operation = Some(OperationState {
            label: label.into(),
            spinner_index: 0,
        });
    }

    pub fn end_operation(&mut self) {
        self.operation = None;
    }

    pub fn is_busy(&self) -> bool {
        self.operation.is_some()
    }

    pub fn advance_spinner(&mut self) {
        if let Some(operation) = self.operation.as_mut() {
            operation.spinner_index = (operation.spinner_index + 1) % SPINNER_FRAMES.len();
        }
    }

    pub fn operation_display(&self) -> Option<String> {
        let operation = self.operation.as_ref()?;
        let frame = SPINNER_FRAMES
            .get(operation.spinner_index)
            .copied()
            .unwrap_or("⢎⡰");
        Some(format!("{frame} {}", operation.label))
    }
}

#[derive(Debug, Clone)]
struct OperationState {
    label: String,
    spinner_index: usize,
}

/// Review submission event kind.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReviewSubmissionEvent {
    Comment,
    Approve,
    RequestChanges,
}

impl ReviewSubmissionEvent {
    pub fn as_api_event(self) -> &'static str {
        match self {
            Self::Comment => "COMMENT",
            Self::Approve => "APPROVE",
            Self::RequestChanges => "REQUEST_CHANGES",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Comment => "Submit Review Comment",
            Self::Approve => "Submit Approval",
            Self::RequestChanges => "Request Changes",
        }
    }
}

/// Thread context resolved from the current selection.
#[derive(Debug, Clone)]
pub struct ThreadActionContext {
    pub root_key: String,
    pub thread_id: Option<String>,
    pub comment_id: u64,
    pub is_resolved: bool,
}

/// Route-local state for the review screen.
#[derive(Debug, Clone)]
pub struct ReviewScreenState {
    pub pull: PullRequestSummary,
    pub data: PullRequestData,
    pub active_tab: ReviewTab,
    pub hide_resolved: bool,
    pub selected_row: usize,
    pub right_scroll: u16,
    pub diff: Option<PullRequestDiffData>,
    pub diff_error: Option<String>,
    pub selected_diff_row: usize,
    pub selected_diff_file: usize,
    pub diff_scroll: u16,
    pub selected_hunk: usize,
    pub diff_viewport_height: u16,
    pub diff_search_focused: bool,
    pub diff_search_query: String,
    pub diff_tree_rows_cache: Vec<DiffTreeRow>,
    pub nodes: Vec<ListNode>,
    pub reply_drafts: HashMap<String, String>,
    collapsed: HashSet<String>,
    diff_collapsed_dirs: HashSet<String>,
    threads_by_key: HashMap<String, ReviewThread>,
}

impl ReviewScreenState {
    pub fn new(pull: PullRequestSummary, data: PullRequestData) -> Self {
        let mut state = Self {
            pull,
            data,
            active_tab: ReviewTab::Threads,
            hide_resolved: true,
            selected_row: 0,
            right_scroll: 0,
            diff: None,
            diff_error: None,
            selected_diff_row: 0,
            selected_diff_file: 0,
            diff_scroll: 0,
            selected_hunk: 0,
            diff_viewport_height: 0,
            diff_search_focused: false,
            diff_search_query: String::new(),
            diff_tree_rows_cache: Vec::new(),
            nodes: Vec::new(),
            reply_drafts: HashMap::new(),
            collapsed: HashSet::new(),
            diff_collapsed_dirs: HashSet::new(),
            threads_by_key: HashMap::new(),
        };

        state.initialize_collapsed_defaults();
        state.rebuild_nodes();
        state
    }

    fn initialize_collapsed_defaults(&mut self) {
        for entry in &self.data.comments {
            if let PullRequestComment::ReviewThread(thread) = entry
                && thread.is_resolved
            {
                self.collapsed.insert(thread_key(thread));
            }
        }
    }

    pub fn rebuild_nodes(&mut self) {
        let selected_key = self.selected_node().map(|node| node.key.clone());
        self.nodes.clear();
        self.threads_by_key.clear();

        let summary_review_ids: HashSet<u64> = self
            .data
            .comments
            .iter()
            .filter_map(|entry| match entry {
                PullRequestComment::ReviewSummary(review) => Some(review.id.into_inner()),
                _ => None,
            })
            .collect();
        let mut grouped_threads: HashMap<u64, Vec<ReviewThread>> = HashMap::new();

        for entry in &self.data.comments {
            let PullRequestComment::ReviewThread(thread) = entry else {
                continue;
            };
            let Some(review_id) = thread
                .comment
                .pull_request_review_id
                .map(|id| id.into_inner())
            else {
                continue;
            };
            if !summary_review_ids.contains(&review_id) {
                continue;
            }
            grouped_threads
                .entry(review_id)
                .or_default()
                .push((**thread).clone());
        }

        for entry in &self.data.comments {
            match entry {
                PullRequestComment::ReviewThread(thread) => {
                    let review_id = thread
                        .comment
                        .pull_request_review_id
                        .map(|id| id.into_inner());
                    if review_id.is_some_and(|id| summary_review_ids.contains(&id)) {
                        continue;
                    }
                    if self.hide_resolved && thread.is_resolved {
                        continue;
                    }
                    append_thread_nodes(
                        &mut self.nodes,
                        &mut self.threads_by_key,
                        &self.collapsed,
                        thread,
                        0,
                    );
                }
                PullRequestComment::IssueComment(comment) => {
                    self.nodes.push(ListNode {
                        key: format!("issue:{}", comment.id),
                        kind: ListNodeKind::Issue,
                        depth: 0,
                        root_key: None,
                        is_resolved: false,
                        is_outdated: false,
                        comment: CommentRef::Issue((**comment).clone()),
                    });
                }
                PullRequestComment::ReviewSummary(review) => {
                    let review_id = review.id.into_inner();
                    let summary_threads = grouped_threads.remove(&review_id).unwrap_or_default();
                    if summary_threads.is_empty() {
                        self.nodes.push(ListNode {
                            key: format!("review-issue:{}", review.id),
                            kind: ListNodeKind::Issue,
                            depth: 0,
                            root_key: None,
                            is_resolved: false,
                            is_outdated: false,
                            comment: CommentRef::ReviewSummary((**review).clone()),
                        });
                        continue;
                    }

                    let has_unresolved_threads =
                        summary_threads.iter().any(|thread| !thread.is_resolved);
                    let group_resolved = !has_unresolved_threads;
                    if self.hide_resolved && group_resolved {
                        continue;
                    }

                    let group_key = review_group_key(review_id);
                    self.nodes.push(ListNode {
                        key: group_key.clone(),
                        kind: ListNodeKind::Review,
                        depth: 0,
                        root_key: None,
                        is_resolved: group_resolved,
                        is_outdated: false,
                        comment: CommentRef::ReviewSummary((**review).clone()),
                    });

                    if self.collapsed.contains(&group_key) {
                        continue;
                    }

                    for thread in &summary_threads {
                        if self.hide_resolved && thread.is_resolved {
                            continue;
                        }
                        append_thread_nodes(
                            &mut self.nodes,
                            &mut self.threads_by_key,
                            &self.collapsed,
                            thread,
                            1,
                        );
                    }
                }
            }
        }

        if let Some(previous_key) = selected_key {
            if let Some(index) = self.nodes.iter().position(|node| node.key == previous_key) {
                self.selected_row = index;
            } else {
                self.selected_row = 0;
            }
        } else {
            self.selected_row = 0;
        }

        if self.selected_row >= self.nodes.len() {
            self.selected_row = self.nodes.len().saturating_sub(1);
        }

        self.right_scroll = 0;
    }

    pub fn selected_node(&self) -> Option<&ListNode> {
        self.nodes.get(self.selected_row)
    }

    pub fn selected_root_thread(&self) -> Option<&ReviewThread> {
        let node = self.selected_node()?;
        let key = node.root_key.as_ref()?;
        self.threads_by_key.get(key)
    }

    pub fn selected_thread_context(&self) -> Option<ThreadActionContext> {
        let node = self.selected_node()?;
        let root_key = node.root_key.as_ref()?.to_owned();
        let thread = self.threads_by_key.get(&root_key)?;

        Some(ThreadActionContext {
            root_key,
            thread_id: thread.thread_id.clone(),
            comment_id: thread.comment.id.into_inner(),
            is_resolved: thread.is_resolved,
        })
    }

    pub fn selected_reply_draft(&self) -> Option<&str> {
        let context = self.selected_thread_context()?;
        self.reply_drafts.get(&context.root_key).map(String::as_str)
    }

    pub fn set_reply_draft(&mut self, root_key: String, body: String) {
        self.reply_drafts.insert(root_key, body);
    }

    pub fn clear_reply_draft(&mut self, root_key: &str) {
        self.reply_drafts.remove(root_key);
    }

    pub fn move_down(&mut self) {
        match self.active_tab {
            ReviewTab::Threads => {
                if self.nodes.is_empty() {
                    self.selected_row = 0;
                    return;
                }

                self.selected_row = (self.selected_row + 1).min(self.nodes.len() - 1);
                self.right_scroll = 0;
            }
            ReviewTab::Diff => {
                let Some((next_row, next_file)) = ({
                    let rows = self.diff_tree_rows();
                    if rows.is_empty() {
                        None
                    } else {
                        let row = (self.selected_diff_row + 1).min(rows.len() - 1);
                        Some((row, rows.get(row).and_then(|value| value.file_index)))
                    }
                }) else {
                    self.selected_diff_row = 0;
                    self.selected_diff_file = 0;
                    return;
                };

                self.selected_diff_row = next_row;
                if let Some(file_index) = next_file {
                    self.set_selected_diff_file(file_index);
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        match self.active_tab {
            ReviewTab::Threads => {
                if self.nodes.is_empty() {
                    self.selected_row = 0;
                    return;
                }

                self.selected_row = self.selected_row.saturating_sub(1);
                self.right_scroll = 0;
            }
            ReviewTab::Diff => {
                let Some((next_row, next_file)) = ({
                    let rows = self.diff_tree_rows();
                    if rows.is_empty() {
                        None
                    } else {
                        let row = self.selected_diff_row.saturating_sub(1);
                        Some((row, rows.get(row).and_then(|value| value.file_index)))
                    }
                }) else {
                    self.selected_diff_row = 0;
                    self.selected_diff_file = 0;
                    return;
                };

                self.selected_diff_row = next_row;
                if let Some(file_index) = next_file {
                    self.set_selected_diff_file(file_index);
                }
            }
        }
    }

    pub fn scroll_preview_down(&mut self) {
        match self.active_tab {
            ReviewTab::Threads => {
                self.right_scroll = self.right_scroll.saturating_add(1);
            }
            ReviewTab::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_add(1);
            }
        }
    }

    pub fn scroll_preview_up(&mut self) {
        match self.active_tab {
            ReviewTab::Threads => {
                self.right_scroll = self.right_scroll.saturating_sub(1);
            }
            ReviewTab::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
        }
    }

    pub fn toggle_selected_thread_collapsed(&mut self) {
        let Some(node) = self.selected_node() else {
            return;
        };

        let is_collapsible_review_group =
            node.kind == ListNodeKind::Review && is_review_group_key(&node.key);
        if node.kind != ListNodeKind::Thread && !is_collapsible_review_group {
            return;
        }
        let key = node.key.clone();

        if self.collapsed.contains(&key) {
            self.collapsed.remove(&key);
        } else {
            self.collapsed.insert(key);
        }

        self.rebuild_nodes();
    }

    pub fn toggle_resolved_filter(&mut self) {
        self.hide_resolved = !self.hide_resolved;
        self.rebuild_nodes();
    }

    pub fn set_data(&mut self, data: PullRequestData) {
        self.data = data;
        self.rebuild_nodes();
    }

    pub fn clear_diff(&mut self) {
        self.diff = None;
        self.diff_error = None;
        self.selected_diff_row = 0;
        self.selected_diff_file = 0;
        self.diff_scroll = 0;
        self.selected_hunk = 0;
        self.diff_viewport_height = 0;
        self.diff_collapsed_dirs.clear();
        self.diff_search_focused = false;
        self.diff_tree_rows_cache.clear();
    }

    pub fn set_diff(&mut self, diff: PullRequestDiffData) {
        self.diff = Some(diff);
        self.diff_error = None;
        self.selected_diff_file = 0;
        self.selected_diff_row = 0;
        self.diff_scroll = 0;
        self.selected_hunk = 0;
        self.diff_viewport_height = 0;
        self.diff_collapsed_dirs.clear();
        self.diff_search_focused = false;
        self.recompute_diff_tree_rows_cache();
        self.select_initial_diff_file_row();
    }

    pub fn set_diff_error(&mut self, error: String) {
        self.diff = None;
        self.diff_error = Some(error);
        self.selected_diff_row = 0;
        self.selected_diff_file = 0;
        self.diff_scroll = 0;
        self.selected_hunk = 0;
        self.diff_viewport_height = 0;
        self.diff_collapsed_dirs.clear();
        self.diff_search_focused = false;
        self.diff_tree_rows_cache.clear();
    }

    pub fn active_tab(&self) -> ReviewTab {
        self.active_tab
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ReviewTab::Threads => ReviewTab::Diff,
            ReviewTab::Diff => ReviewTab::Threads,
        };
    }

    pub fn prev_tab(&mut self) {
        self.next_tab();
    }

    pub fn selected_diff_file(&self) -> Option<&PullRequestDiffFile> {
        let diff = self.diff.as_ref()?;
        diff.files.get(self.selected_diff_file)
    }

    pub fn diff_file_count(&self) -> usize {
        self.diff
            .as_ref()
            .map(|value| value.files.len())
            .unwrap_or(0)
    }

    pub fn diff_tree_rows(&self) -> &[DiffTreeRow] {
        &self.diff_tree_rows_cache
    }

    pub fn selected_diff_tree_row(&self) -> usize {
        self.selected_diff_row
    }

    pub fn toggle_selected_diff_directory_collapsed(&mut self) {
        let Some((row_key, is_directory)) = ({
            let rows = self.diff_tree_rows();
            rows.get(self.selected_diff_row)
                .map(|row| (row.key.clone(), row.is_directory))
        }) else {
            return;
        };
        if !is_directory {
            return;
        }

        if self.diff_collapsed_dirs.contains(&row_key) {
            self.diff_collapsed_dirs.remove(&row_key);
        } else {
            self.diff_collapsed_dirs.insert(row_key.clone());
        }
        self.recompute_diff_tree_rows_cache();

        let Some((row_index, file_index)) = ({
            let rows = self.diff_tree_rows();
            if rows.is_empty() {
                None
            } else {
                let row_index = rows
                    .iter()
                    .position(|candidate| candidate.key == row_key)
                    .unwrap_or_else(|| self.selected_diff_row.min(rows.len().saturating_sub(1)));
                Some((
                    row_index,
                    rows.get(row_index).and_then(|row| row.file_index),
                ))
            }
        }) else {
            self.selected_diff_row = 0;
            self.selected_diff_file = 0;
            self.diff_scroll = 0;
            self.selected_hunk = 0;
            return;
        };

        self.selected_diff_row = row_index;
        if let Some(file_index) = file_index {
            self.set_selected_diff_file(file_index);
        } else {
            self.selected_diff_file = 0;
            self.diff_scroll = 0;
            self.selected_hunk = 0;
        }
    }

    pub fn focus_diff_search(&mut self) {
        self.diff_search_focused = true;
    }

    pub fn unfocus_diff_search(&mut self) {
        self.diff_search_focused = false;
    }

    pub fn is_diff_search_focused(&self) -> bool {
        self.diff_search_focused
    }

    pub fn diff_search_query(&self) -> &str {
        &self.diff_search_query
    }

    pub fn diff_search_push_char(&mut self, ch: char) {
        self.diff_search_query.push(ch);
        self.recompute_diff_tree_rows_cache();
        self.realign_diff_selection_for_filter();
    }

    pub fn diff_search_backspace(&mut self) {
        self.diff_search_query.pop();
        self.recompute_diff_tree_rows_cache();
        self.realign_diff_selection_for_filter();
    }

    pub fn jump_next_hunk(&mut self) {
        let Some(diff) = self.diff.as_ref() else {
            return;
        };
        let files = self.navigable_diff_files();
        if files.is_empty() {
            return;
        }

        let current_file = self.selected_diff_file;
        let current = usize::from(self.diff_scroll);
        let Some(current_pos) = files.iter().position(|index| *index == current_file) else {
            return;
        };

        if let Some(file) = diff.files.get(current_file)
            && let Some(next_hunk) = file
                .hunk_starts
                .iter()
                .copied()
                .find(|start| *start > current)
        {
            let hunk_index = file
                .hunk_starts
                .iter()
                .position(|start| *start == next_hunk)
                .unwrap_or(0);
            self.set_active_hunk(current_file, next_hunk, hunk_index);
            return;
        }

        for offset in 1..files.len() {
            let file_index = files[(current_pos + offset) % files.len()];
            let Some(file) = diff.files.get(file_index) else {
                continue;
            };
            if let Some(next_hunk) = file.hunk_starts.first().copied() {
                self.set_active_hunk(file_index, next_hunk, 0);
                return;
            }
        }

        if let Some(file) = diff.files.get(current_file)
            && let Some(next_hunk) = file.hunk_starts.first().copied()
        {
            self.set_active_hunk(current_file, next_hunk, 0);
        }
    }

    pub fn jump_prev_hunk(&mut self) {
        let Some(diff) = self.diff.as_ref() else {
            return;
        };
        let files = self.navigable_diff_files();
        if files.is_empty() {
            return;
        }

        let current_file = self.selected_diff_file;
        let current = usize::from(self.diff_scroll);
        let Some(current_pos) = files.iter().position(|index| *index == current_file) else {
            return;
        };

        if let Some(file) = diff.files.get(current_file)
            && let Some(previous_hunk) = file
                .hunk_starts
                .iter()
                .copied()
                .rev()
                .find(|start| *start < current)
        {
            let hunk_index = file
                .hunk_starts
                .iter()
                .position(|start| *start == previous_hunk)
                .unwrap_or(0);
            self.set_active_hunk(current_file, previous_hunk, hunk_index);
            return;
        }

        for offset in 1..files.len() {
            let file_index = files[(current_pos + files.len() - offset) % files.len()];
            let Some(file) = diff.files.get(file_index) else {
                continue;
            };
            if let Some(previous_hunk) = file.hunk_starts.last().copied() {
                let hunk_index = file.hunk_starts.len().saturating_sub(1);
                self.set_active_hunk(file_index, previous_hunk, hunk_index);
                return;
            }
        }

        if let Some(file) = diff.files.get(current_file)
            && let Some(previous_hunk) = file.hunk_starts.last().copied()
        {
            let hunk_index = file.hunk_starts.len().saturating_sub(1);
            self.set_active_hunk(current_file, previous_hunk, hunk_index);
        }
    }

    pub fn is_collapsed(&self, key: &str) -> bool {
        self.collapsed.contains(key)
    }

    pub fn set_diff_viewport_height(&mut self, height: u16) {
        self.diff_viewport_height = height.max(1);
    }

    fn set_selected_diff_file(&mut self, file_index: usize) {
        if file_index == self.selected_diff_file {
            return;
        }

        self.selected_diff_file = file_index;
        self.diff_scroll = self.first_hunk_scroll(file_index);
        self.selected_hunk = 0;
    }

    fn first_hunk_scroll(&self, file_index: usize) -> u16 {
        self.diff
            .as_ref()
            .and_then(|diff| diff.files.get(file_index))
            .and_then(|file| file.hunk_starts.first().copied())
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0)
    }

    fn realign_diff_selection_for_filter(&mut self) {
        let Some((row_index, file_index)) = ({
            let rows = self.diff_tree_rows();
            if rows.is_empty() {
                None
            } else {
                let current_file = rows
                    .get(self.selected_diff_row)
                    .and_then(|row| row.file_index)
                    .or_else(|| {
                        rows.iter()
                            .position(|row| row.file_index == Some(self.selected_diff_file))
                            .and_then(|index| rows.get(index).and_then(|row| row.file_index))
                    });

                if let Some(file_index) = current_file
                    && let Some(index) = rows
                        .iter()
                        .position(|row| row.file_index == Some(file_index))
                {
                    Some((index, Some(file_index)))
                } else {
                    rows.iter().enumerate().find_map(|(index, row)| {
                        row.file_index.map(|file_index| (index, Some(file_index)))
                    })
                }
            }
        }) else {
            self.selected_diff_row = 0;
            self.selected_diff_file = 0;
            self.diff_scroll = 0;
            self.selected_hunk = 0;
            return;
        };

        self.selected_diff_row = row_index;
        if let Some(file_index) = file_index {
            self.set_selected_diff_file(file_index);
        } else {
            self.selected_diff_row = 0;
            self.selected_diff_file = 0;
            self.diff_scroll = 0;
            self.selected_hunk = 0;
        }
    }

    fn select_initial_diff_file_row(&mut self) {
        let Some((row_index, file_index)) = self
            .diff_tree_rows()
            .iter()
            .enumerate()
            .find_map(|(index, row)| row.file_index.map(|file_index| (index, file_index)))
        else {
            return;
        };

        self.selected_diff_row = row_index;
        self.selected_diff_file = file_index;
        self.selected_hunk = 0;
        self.diff_scroll = self.first_hunk_scroll(file_index);
    }

    fn set_active_hunk(&mut self, file_index: usize, hunk_start: usize, hunk_index: usize) {
        self.ensure_diff_file_expanded(file_index);
        self.selected_diff_file = file_index;
        self.selected_hunk = hunk_index;
        let viewport_height = usize::from(self.diff_viewport_height.max(1));
        let centered_start = hunk_start.saturating_sub(viewport_height / 2);
        let clamped_scroll = self
            .diff
            .as_ref()
            .and_then(|diff| diff.files.get(file_index))
            .map(|file| {
                if file.rows.is_empty() {
                    centered_start
                } else {
                    centered_start.min(file.rows.len().saturating_sub(viewport_height))
                }
            })
            .unwrap_or(centered_start);
        self.diff_scroll = u16::try_from(clamped_scroll).unwrap_or(u16::MAX);

        if let Some(row_index) = self
            .diff_tree_rows()
            .iter()
            .position(|row| row.file_index == Some(file_index))
        {
            self.selected_diff_row = row_index;
        }
    }

    fn ensure_diff_file_expanded(&mut self, file_index: usize) {
        if !self.diff_search_query.trim().is_empty() {
            return;
        }

        let Some(path) = self
            .diff
            .as_ref()
            .and_then(|diff| diff.files.get(file_index))
            .map(|file| file.path.clone())
        else {
            return;
        };

        let mut key = String::new();
        let parts = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();

        let mut changed = false;
        for segment in parts.iter().take(parts.len().saturating_sub(1)) {
            if !key.is_empty() {
                key.push('/');
            }
            key.push_str(segment);
            changed |= self.diff_collapsed_dirs.remove(&key);
        }

        if changed {
            self.recompute_diff_tree_rows_cache();
        }
    }

    fn navigable_diff_files(&self) -> Vec<usize> {
        let rows = self.diff_tree_rows();
        let mut seen = HashSet::new();
        let mut files = Vec::new();
        for row in rows {
            let Some(file_index) = row.file_index else {
                continue;
            };
            if seen.insert(file_index) {
                files.push(file_index);
            }
        }

        if !files.is_empty() {
            return files;
        }

        self.diff
            .as_ref()
            .map(|diff| {
                diff.files
                    .iter()
                    .enumerate()
                    .filter_map(|(index, file)| (!file.hunk_starts.is_empty()).then_some(index))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn recompute_diff_tree_rows_cache(&mut self) {
        let Some(diff) = self.diff.as_ref() else {
            self.diff_tree_rows_cache.clear();
            return;
        };

        self.diff_tree_rows_cache = if self.diff_search_query.trim().is_empty() {
            build_diff_tree_rows(diff, &self.diff_collapsed_dirs)
        } else {
            let expanded_rows = build_diff_tree_rows(diff, &HashSet::new());
            filter_diff_tree_rows(&expanded_rows, diff, &self.diff_search_query)
        };
    }
}

/// Active sub-view in the review screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReviewTab {
    Threads,
    Diff,
}

#[derive(Debug, Clone)]
pub struct DiffTreeRow {
    pub key: String,
    pub label: String,
    pub depth: usize,
    pub is_directory: bool,
    pub is_collapsed: bool,
    pub file_index: Option<usize>,
}

fn append_reply_nodes(
    nodes: &mut Vec<ListNode>,
    thread: &ReviewThread,
    depth: usize,
    root_key: &str,
) {
    for reply in &thread.replies {
        nodes.push(ListNode {
            key: format!("reply:{}", reply.comment.id.into_inner()),
            kind: ListNodeKind::Reply,
            depth,
            root_key: Some(root_key.to_owned()),
            is_resolved: thread.is_resolved,
            is_outdated: review_comment_is_outdated(&reply.comment),
            comment: CommentRef::Review(reply.comment.clone()),
        });

        append_reply_nodes(nodes, reply, depth + 1, root_key);
    }
}

fn append_thread_nodes(
    nodes: &mut Vec<ListNode>,
    threads_by_key: &mut HashMap<String, ReviewThread>,
    collapsed: &HashSet<String>,
    thread: &ReviewThread,
    depth: usize,
) {
    let key = thread_key(thread);
    threads_by_key.insert(key.clone(), thread.clone());

    nodes.push(ListNode {
        key: key.clone(),
        kind: ListNodeKind::Thread,
        depth,
        root_key: Some(key.clone()),
        is_resolved: thread.is_resolved,
        is_outdated: review_comment_is_outdated(&thread.comment),
        comment: CommentRef::Review(thread.comment.clone()),
    });

    if collapsed.contains(&key) {
        return;
    }

    append_reply_nodes(nodes, thread, depth + 1, &key);
}

fn thread_key(thread: &ReviewThread) -> String {
    match thread
        .thread_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(id) => format!("thread:{id}"),
        None => format!("comment:{}", thread.comment.id.into_inner()),
    }
}

fn review_group_key(review_id: u64) -> String {
    format!("review-group:{review_id}")
}

fn is_review_group_key(key: &str) -> bool {
    key.starts_with("review-group:")
}

#[derive(Debug, Default, Clone)]
struct DiffTreeNode {
    children: BTreeMap<String, DiffTreeNode>,
    files: Vec<usize>,
}

fn build_diff_tree_rows(
    diff: &PullRequestDiffData,
    collapsed: &HashSet<String>,
) -> Vec<DiffTreeRow> {
    let mut root = DiffTreeNode::default();

    for (index, file) in diff.files.iter().enumerate() {
        insert_diff_path(&mut root, &file.path, index);
    }
    sort_diff_tree_files(&mut root, diff);

    let mut rows = Vec::new();
    append_diff_tree_rows(&root, "", 0, &mut rows, diff, collapsed);
    rows
}

fn filter_diff_tree_rows(
    rows: &[DiffTreeRow],
    diff: &PullRequestDiffData,
    query: &str,
) -> Vec<DiffTreeRow> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return rows.to_vec();
    }

    let mut include = HashSet::<String>::new();
    let mut parent_stack = Vec::<String>::new();

    for row in rows {
        while parent_stack.len() > row.depth {
            parent_stack.pop();
        }

        if row.is_directory {
            parent_stack.push(row.key.clone());
            continue;
        }

        let matches = row
            .file_index
            .and_then(|file_index| diff.files.get(file_index))
            .map(|file| file.path.to_ascii_lowercase().contains(&query))
            .unwrap_or(false);

        if matches {
            include.insert(row.key.clone());
            for key in &parent_stack {
                include.insert(key.clone());
            }
        }
    }

    rows.iter()
        .filter(|row| include.contains(&row.key))
        .cloned()
        .map(|mut row| {
            row.is_collapsed = false;
            row
        })
        .collect()
}

fn insert_diff_path(root: &mut DiffTreeNode, path: &str, file_index: usize) {
    let parts = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        root.files.push(file_index);
        return;
    }

    if parts.len() == 1 {
        root.files.push(file_index);
        return;
    }

    let mut node = root;
    for segment in &parts[..parts.len() - 1] {
        node = node.children.entry((*segment).to_owned()).or_default();
    }
    node.files.push(file_index);
}

fn sort_diff_tree_files(node: &mut DiffTreeNode, diff: &PullRequestDiffData) {
    node.files
        .sort_by(|a, b| diff.files[*a].path.cmp(&diff.files[*b].path));
    for child in node.children.values_mut() {
        sort_diff_tree_files(child, diff);
    }
}

fn append_diff_tree_rows(
    node: &DiffTreeNode,
    parent_key: &str,
    depth: usize,
    rows: &mut Vec<DiffTreeRow>,
    diff: &PullRequestDiffData,
    collapsed: &HashSet<String>,
) {
    for (segment, child) in &node.children {
        let (dir_label, dir_key, compressed) = compress_directory(parent_key, segment, child);
        let is_collapsed = collapsed.contains(&dir_key);

        rows.push(DiffTreeRow {
            key: dir_key.clone(),
            label: dir_label,
            depth,
            is_directory: true,
            is_collapsed,
            file_index: None,
        });

        if !is_collapsed {
            append_diff_tree_rows(compressed, &dir_key, depth + 1, rows, diff, collapsed);
        }
    }

    for file_index in &node.files {
        let file = &diff.files[*file_index];
        let label = file
            .path
            .rsplit('/')
            .next()
            .unwrap_or(file.path.as_str())
            .to_owned();
        rows.push(DiffTreeRow {
            key: format!("file:{}", file.path),
            label,
            depth,
            is_directory: false,
            is_collapsed: false,
            file_index: Some(*file_index),
        });
    }
}

fn compress_directory<'a>(
    parent_key: &str,
    initial_segment: &str,
    initial_node: &'a DiffTreeNode,
) -> (String, String, &'a DiffTreeNode) {
    let mut label = initial_segment.to_owned();
    let mut key = join_path(parent_key, initial_segment);
    let mut node = initial_node;

    while node.files.is_empty() && node.children.len() == 1 {
        let Some((segment, next)) = node.children.iter().next() else {
            break;
        };
        label.push('/');
        label.push_str(segment);
        key.push('/');
        key.push_str(segment);
        node = next;
    }

    (label, key, node)
}

fn join_path(parent: &str, segment: &str) -> String {
    if parent.is_empty() {
        segment.to_owned()
    } else {
        format!("{parent}/{segment}")
    }
}

#[cfg(test)]
mod tests {
    use super::{ReviewScreenState, ReviewTab, build_diff_tree_rows};
    use crate::domain::{
        PullRequestData, PullRequestDiffData, PullRequestDiffFile, PullRequestDiffFileStatus,
        PullRequestDiffRow, PullRequestDiffRowKind, PullRequestSummary,
    };
    use std::collections::HashSet;

    fn diff_file(path: &str) -> PullRequestDiffFile {
        PullRequestDiffFile {
            path: path.to_owned(),
            status: PullRequestDiffFileStatus::Modified,
            rows: Vec::new(),
            hunk_starts: Vec::new(),
        }
    }

    fn context_row() -> PullRequestDiffRow {
        PullRequestDiffRow {
            left_line_number: None,
            right_line_number: None,
            left_text: String::new(),
            right_text: String::new(),
            left_highlights: Vec::new(),
            right_highlights: Vec::new(),
            kind: PullRequestDiffRowKind::Context,
        }
    }

    fn build_review_state() -> ReviewScreenState {
        let pull = PullRequestSummary {
            owner: "owner".to_owned(),
            repo: "repo".to_owned(),
            number: 42,
            title: "Example".to_owned(),
            author: "dev".to_owned(),
            head_ref: "feature".to_owned(),
            base_ref: "main".to_owned(),
            head_sha: "headsha".to_owned(),
            base_sha: "basesha".to_owned(),
            html_url: Some("https://example.com".to_owned()),
            updated_at: "now".to_owned(),
            updated_at_unix_ms: 0,
            review_status: None,
        };

        let data = PullRequestData {
            owner: "owner".to_owned(),
            repo: "repo".to_owned(),
            pull_number: 42,
            head_ref: "feature".to_owned(),
            base_ref: "main".to_owned(),
            head_sha: "headsha".to_owned(),
            base_sha: "basesha".to_owned(),
            changed_files: Vec::new(),
            comments: Vec::new(),
        };

        ReviewScreenState::new(pull, data)
    }

    #[test]
    fn compresses_single_directory_chain() {
        let diff = PullRequestDiffData {
            files: vec![
                diff_file("one/two/three/a.rs"),
                diff_file("one/two/three/b.rs"),
                diff_file("one/two/three/c.rs"),
            ],
        };

        let rows = build_diff_tree_rows(&diff, &HashSet::new());
        assert!(!rows.is_empty());
        assert!(rows[0].is_directory);
        assert_eq!(rows[0].label, "one/two/three");
        assert_eq!(rows[0].depth, 0);
    }

    #[test]
    fn does_not_compress_across_branching_directory() {
        let diff = PullRequestDiffData {
            files: vec![diff_file("one/two/a.rs"), diff_file("one/three/b.rs")],
        };

        let rows = build_diff_tree_rows(&diff, &HashSet::new());
        assert!(!rows.is_empty());
        assert!(rows[0].is_directory);
        assert_eq!(rows[0].label, "one");
    }

    #[test]
    fn set_diff_selects_first_tree_file() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        let mut second = diff_file("beta.rs");
        first.hunk_starts = vec![];
        second.hunk_starts = vec![12, 40];

        review.set_diff(PullRequestDiffData {
            files: vec![first, second],
        });

        assert_eq!(review.selected_diff_file, 0);
        assert_eq!(review.diff_scroll, 0);
        assert_eq!(review.selected_hunk, 0);
    }

    #[test]
    fn set_diff_scrolls_to_first_hunk_in_first_tree_file() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        let mut second = diff_file("beta.rs");
        first.hunk_starts = vec![8, 30];
        second.hunk_starts = vec![3];

        review.set_diff(PullRequestDiffData {
            files: vec![first, second],
        });

        assert_eq!(review.selected_diff_file, 0);
        assert_eq!(review.diff_scroll, 8);
        assert_eq!(review.selected_hunk, 0);
    }

    #[test]
    fn jump_next_hunk_wraps_to_next_file() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        let mut second = diff_file("beta.rs");
        first.hunk_starts = vec![4, 9];
        second.hunk_starts = vec![3];

        review.set_diff(PullRequestDiffData {
            files: vec![first, diff_file("no_hunks.rs"), second],
        });
        review.selected_diff_file = 0;
        review.diff_scroll = 9;

        review.jump_next_hunk();

        assert_eq!(review.selected_diff_file, 2);
        assert_eq!(review.diff_scroll, 3);
        assert_eq!(review.selected_hunk, 0);
    }

    #[test]
    fn jump_prev_hunk_wraps_to_previous_file() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        let mut second = diff_file("beta.rs");
        first.hunk_starts = vec![4, 9];
        second.hunk_starts = vec![3, 8];

        review.set_diff(PullRequestDiffData {
            files: vec![first, diff_file("no_hunks.rs"), second],
        });
        review.selected_diff_file = 0;
        review.diff_scroll = 4;

        review.jump_prev_hunk();

        assert_eq!(review.selected_diff_file, 2);
        assert_eq!(review.diff_scroll, 8);
        assert_eq!(review.selected_hunk, 1);
    }

    #[test]
    fn jump_next_hunk_centers_target_in_viewport() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        first.hunk_starts = vec![60];
        first.rows = vec![context_row(); 200];

        review.set_diff(PullRequestDiffData { files: vec![first] });
        review.set_diff_viewport_height(20);
        review.diff_scroll = 0;

        review.jump_next_hunk();

        assert_eq!(review.selected_diff_file, 0);
        assert_eq!(review.selected_hunk, 0);
        assert_eq!(review.diff_scroll, 50);
    }

    #[test]
    fn move_file_selection_jumps_to_first_hunk() {
        let mut review = build_review_state();
        let mut first = diff_file("alpha.rs");
        let mut second = diff_file("beta.rs");
        first.hunk_starts = vec![4, 20];
        second.hunk_starts = vec![13, 44];

        review.set_diff(PullRequestDiffData {
            files: vec![first, second],
        });
        review.active_tab = ReviewTab::Diff;

        review.move_down();
        assert_eq!(review.selected_diff_file, 1);
        assert_eq!(review.selected_hunk, 0);
        assert_eq!(review.diff_scroll, 13);

        review.move_up();
        assert_eq!(review.selected_diff_file, 0);
        assert_eq!(review.selected_hunk, 0);
        assert_eq!(review.diff_scroll, 4);
    }

    #[test]
    fn diff_search_filters_rows_and_updates_selection() {
        let mut review = build_review_state();
        review.set_diff(PullRequestDiffData {
            files: vec![diff_file("src/lib.rs"), diff_file("docs/readme.md")],
        });

        review.diff_search_push_char('d');
        review.diff_search_push_char('o');

        let rows = review.diff_tree_rows();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| {
            row.is_directory || row.key.contains("docs/readme.md") || row.key.contains("docs")
        }));
        assert_eq!(review.selected_diff_file, 1);
    }
}
