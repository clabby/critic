//! Application state models and route-local behavior.

use crate::domain::{
    CommentRef, ListNode, ListNodeKind, PullRequestComment, PullRequestData, PullRequestSummary,
    ReviewThread, Route, review_comment_is_outdated,
};
use crate::search::fuzzy::rank_pull_requests;
use std::collections::{HashMap, HashSet};

/// Spinner frames used for active async operations.
pub const SPINNER_FRAMES: [char; 4] = ['|', '/', '-', '\\'];

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
            .unwrap_or('|');
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
    pub hide_resolved: bool,
    pub selected_row: usize,
    pub right_scroll: u16,
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
            hide_resolved: true,
            selected_row: 0,
            right_scroll: 0,
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

        for entry in &self.data.comments {
            match entry {
                PullRequestComment::ReviewThread(thread) => {
                    let key = thread_key(thread);
                    self.threads_by_key.insert(key.clone(), (**thread).clone());

                    if self.hide_resolved && thread.is_resolved {
                        continue;
                    }

                    self.nodes.push(ListNode {
                        key: key.clone(),
                        kind: ListNodeKind::Thread,
                        depth: 0,
                        root_key: Some(key.clone()),
                        is_resolved: thread.is_resolved,
                        is_outdated: review_comment_is_outdated(&thread.comment),
                        comment: CommentRef::Review(thread.comment.clone()),
                    });

                    if self.collapsed.contains(&key) {
                        continue;
                    }

                    append_reply_nodes(&mut self.nodes, thread, 1, &key);
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
            comment_id: thread.comment.id,
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
        if self.nodes.is_empty() {
            self.selected_row = 0;
            return;
        }

        self.selected_row = (self.selected_row + 1).min(self.nodes.len() - 1);
        self.right_scroll = 0;
    }

    pub fn move_up(&mut self) {
        if self.nodes.is_empty() {
            self.selected_row = 0;
            return;
        }

        self.selected_row = self.selected_row.saturating_sub(1);
        self.right_scroll = 0;
    }

    pub fn scroll_preview_down(&mut self) {
        self.right_scroll = self.right_scroll.saturating_add(1);
    }

    pub fn scroll_preview_up(&mut self) {
        self.right_scroll = self.right_scroll.saturating_sub(1);
    }

    pub fn toggle_selected_thread_collapsed(&mut self) {
        let Some(node) = self.selected_node() else {
            return;
        };

        if node.kind != ListNodeKind::Thread {
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

    pub fn is_collapsed(&self, key: &str) -> bool {
        self.collapsed.contains(key)
    }
}

fn append_reply_nodes(
    nodes: &mut Vec<ListNode>,
    thread: &ReviewThread,
    depth: usize,
    root_key: &str,
) {
    for reply in &thread.replies {
        nodes.push(ListNode {
            key: format!("reply:{}", reply.comment.id),
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

fn thread_key(thread: &ReviewThread) -> String {
    match thread
        .thread_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(id) => format!("thread:{id}"),
        None => format!("comment:{}", thread.comment.id),
    }
}
