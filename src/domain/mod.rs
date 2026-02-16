//! Domain models shared across GitHub, search, and UI layers.

use octocrab::models::{issues, pulls};
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

/// A pull request review comment from GitHub API.
pub type ReviewComment = pulls::Comment;

/// An issue-style pull request comment from GitHub API.
pub type IssueComment = issues::Comment;

/// A pull request review summary from GitHub API.
pub type PullReviewSummary = pulls::Review;

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
    ReviewSummary(Box<PullReviewSummary>),
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
    Review,
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
    ReviewSummary(PullReviewSummary),
}

impl CommentRef {
    pub fn author(&self) -> &str {
        match self {
            Self::Review(comment) => comment
                .user
                .as_ref()
                .map(|user| user.login.as_str())
                .unwrap_or("unknown"),
            Self::Issue(comment) => comment.user.login.as_str(),
            Self::ReviewSummary(review) => review
                .user
                .as_ref()
                .map(|user| user.login.as_str())
                .unwrap_or("unknown"),
        }
    }

    pub fn body(&self) -> &str {
        match self {
            Self::Review(comment) => comment.body.as_str(),
            Self::Issue(comment) => comment.body.as_deref().unwrap_or(""),
            Self::ReviewSummary(review) => review.body.as_deref().unwrap_or(""),
        }
    }
}

/// Returns whether a review comment no longer has a usable source location.
pub fn review_comment_is_outdated(comment: &ReviewComment) -> bool {
    let has_path = !comment.path.trim().is_empty();
    let has_line = comment.line.or(comment.start_line).is_some();
    !(has_path && has_line)
}

impl fmt::Display for ListNodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thread => write!(f, "thread"),
            Self::Reply => write!(f, "reply"),
            Self::Issue => write!(f, "issue"),
            Self::Review => write!(f, "review"),
        }
    }
}
