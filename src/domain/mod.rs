//! Domain models shared across GitHub, search, and UI layers.

use std::fmt;

/// A lightweight pull request summary shown on the search screen.
#[derive(Debug, Clone)]
pub struct PullRequestSummary {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub author: String,
    pub head_ref: String,
    pub base_ref: String,
    pub html_url: Option<String>,
    pub updated_at: String,
    pub updated_at_unix_ms: i64,
}

impl PullRequestSummary {
    /// Returns a searchable composite string used by fuzzy matching.
    pub fn search_text(&self) -> String {
        format!(
            "#{} {} @{} {} -> {}",
            self.number, self.title, self.author, self.head_ref, self.base_ref
        )
    }

    /// Returns a compact UI label.
    pub fn display_line(&self) -> String {
        format!("#{} {} (@{})", self.number, self.title, self.author)
    }
}

/// A pull request review comment from `/pulls/comments`.
#[derive(Debug, Clone)]
pub struct ReviewComment {
    pub id: u64,
    pub in_reply_to_id: Option<u64>,
    pub body: String,
    pub diff_hunk: Option<String>,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub start_line: Option<u64>,
    pub original_line: Option<u64>,
    pub original_start_line: Option<u64>,
    pub side: Option<String>,
    pub html_url: Option<String>,
    pub created_at: String,
    pub author: String,
}

/// An issue-style pull request comment from `/issues/comments`.
#[derive(Debug, Clone)]
pub struct IssueComment {
    pub id: u64,
    pub body: String,
    pub html_url: Option<String>,
    pub created_at: String,
    pub author: String,
}

/// A hierarchical review thread rooted at a top-level review comment.
#[derive(Debug, Clone)]
pub struct ReviewThread {
    pub thread_id: Option<String>,
    pub is_resolved: bool,
    pub comment: ReviewComment,
    pub replies: Vec<ReviewThread>,
}

impl ReviewThread {
    pub fn total_comments(&self) -> usize {
        1 + self
            .replies
            .iter()
            .map(ReviewThread::total_comments)
            .sum::<usize>()
    }
}

/// A merged comment entry shown in the left pane.
#[derive(Debug, Clone)]
pub enum PullRequestComment {
    ReviewThread(Box<ReviewThread>),
    IssueComment(Box<IssueComment>),
}

/// All review data required for the review screen.
#[derive(Debug, Clone)]
pub struct PullRequestData {
    pub owner: String,
    pub repo: String,
    pub pull_number: u64,
    pub head_ref: String,
    pub base_ref: String,
    pub changed_files: Vec<String>,
    pub comments: Vec<PullRequestComment>,
}

impl PullRequestData {
    pub fn review_thread_totals(&self) -> (usize, usize) {
        let mut total = 0usize;
        let mut resolved = 0usize;

        for entry in &self.comments {
            if let PullRequestComment::ReviewThread(thread) = entry {
                total += 1;
                if thread.is_resolved {
                    resolved += 1;
                }
            }
        }

        (resolved, total)
    }
}

/// The current application route.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Route {
    Search,
    Review,
}

/// A flattened left-pane row kind.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ListNodeKind {
    Thread,
    Reply,
    Issue,
}

/// A flattened left-pane row for navigation/rendering.
#[derive(Debug, Clone)]
pub struct ListNode {
    pub key: String,
    pub kind: ListNodeKind,
    pub depth: usize,
    pub root_key: Option<String>,
    pub is_resolved: bool,
    pub is_outdated: bool,
    pub comment: CommentRef,
}

/// Union of comment variants attached to a list node.
#[derive(Debug, Clone)]
pub enum CommentRef {
    Review(ReviewComment),
    Issue(IssueComment),
}

impl CommentRef {
    pub fn author(&self) -> &str {
        match self {
            Self::Review(comment) => &comment.author,
            Self::Issue(comment) => &comment.author,
        }
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Review(comment) => &comment.body,
            Self::Issue(comment) => &comment.body,
        }
    }

    pub fn html_url(&self) -> Option<&str> {
        match self {
            Self::Review(comment) => comment.html_url.as_deref(),
            Self::Issue(comment) => comment.html_url.as_deref(),
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Review(comment) => comment.path.as_deref(),
            Self::Issue(_) => None,
        }
    }

    pub fn line(&self) -> Option<u64> {
        match self {
            Self::Review(comment) => comment.line.or(comment.start_line),
            Self::Issue(_) => None,
        }
    }

    pub fn created_at(&self) -> &str {
        match self {
            Self::Review(comment) => &comment.created_at,
            Self::Issue(comment) => &comment.created_at,
        }
    }
}

/// Returns whether a review comment no longer has a usable source location.
pub fn review_comment_is_outdated(comment: &ReviewComment) -> bool {
    let has_path = comment
        .path
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let has_line = comment.line.or(comment.start_line).is_some();
    !(has_path && has_line)
}

impl fmt::Display for ListNodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thread => write!(f, "thread"),
            Self::Reply => write!(f, "reply"),
            Self::Issue => write!(f, "issue"),
        }
    }
}
