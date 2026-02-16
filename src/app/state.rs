//! Application state models and route-local behavior.

use crate::domain::{
    CommentRef, ListNode, ListNodeKind, PullRequestComment, PullRequestData, PullRequestDiffData,
    PullRequestDiffFile, PullRequestSummary, ReviewThread, Route, review_comment_is_outdated,
};
use crate::search::fuzzy::rank_pull_requests;
use std::collections::{HashMap, HashSet};

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
    pub selected_diff_file: usize,
    pub diff_scroll: u16,
    pub selected_hunk: usize,
    pub nodes: Vec<ListNode>,
    pub reply_drafts: HashMap<String, String>,
    collapsed: HashSet<String>,
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
            selected_diff_file: 0,
            diff_scroll: 0,
            selected_hunk: 0,
            nodes: Vec::new(),
            reply_drafts: HashMap::new(),
            collapsed: HashSet::new(),
            threads_by_key: HashMap::new(),
        };

        state.initialize_collapsed_defaults();
        state.rebuild_nodes();
        state
    }

    fn initialize_collapsed_defaults(&mut self) {
        for entry in &self.data.comments {
            if let PullRequestComment::ReviewThread(thread) = entry {
                if thread.is_resolved {
                    self.collapsed.insert(thread_key(thread));
                }
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
                let file_count = self.diff.as_ref().map(|diff| diff.files.len()).unwrap_or(0);
                if file_count == 0 {
                    self.selected_diff_file = 0;
                    return;
                }
                self.selected_diff_file = (self.selected_diff_file + 1).min(file_count - 1);
                self.diff_scroll = 0;
                self.selected_hunk = 0;
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
                let file_count = self.diff.as_ref().map(|diff| diff.files.len()).unwrap_or(0);
                if file_count == 0 {
                    self.selected_diff_file = 0;
                    return;
                }
                self.selected_diff_file = self.selected_diff_file.saturating_sub(1);
                self.diff_scroll = 0;
                self.selected_hunk = 0;
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

    pub fn set_diff(&mut self, diff: PullRequestDiffData) {
        self.diff = Some(diff);
        self.diff_error = None;
        self.selected_diff_file = 0;
        self.diff_scroll = 0;
        self.selected_hunk = 0;
    }

    pub fn set_diff_error(&mut self, error: String) {
        self.diff = None;
        self.diff_error = Some(error);
        self.selected_diff_file = 0;
        self.diff_scroll = 0;
        self.selected_hunk = 0;
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

    pub fn jump_next_hunk(&mut self) {
        let Some(hunk_starts) = self
            .selected_diff_file()
            .map(|file| file.hunk_starts.clone())
        else {
            return;
        };
        if hunk_starts.is_empty() {
            return;
        }

        let current = usize::from(self.diff_scroll);
        let next = hunk_starts
            .iter()
            .copied()
            .find(|start| *start > current)
            .unwrap_or(hunk_starts[0]);
        self.diff_scroll = u16::try_from(next).unwrap_or(u16::MAX);
        self.selected_hunk = hunk_starts
            .iter()
            .position(|start| *start == next)
            .unwrap_or(0);
    }

    pub fn jump_prev_hunk(&mut self) {
        let Some(hunk_starts) = self
            .selected_diff_file()
            .map(|file| file.hunk_starts.clone())
        else {
            return;
        };
        if hunk_starts.is_empty() {
            return;
        }

        let current = usize::from(self.diff_scroll);
        let previous = hunk_starts
            .iter()
            .copied()
            .rev()
            .find(|start| *start < current)
            .unwrap_or_else(|| *hunk_starts.last().unwrap_or(&0));
        self.diff_scroll = u16::try_from(previous).unwrap_or(u16::MAX);
        self.selected_hunk = hunk_starts
            .iter()
            .position(|start| *start == previous)
            .unwrap_or(0);
    }

    pub fn is_collapsed(&self, key: &str) -> bool {
        self.collapsed.contains(key)
    }
}

/// Active sub-view in the review screen.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReviewTab {
    Threads,
    Diff,
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
