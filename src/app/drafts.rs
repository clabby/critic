//! Persistent storage for in-progress review drafts.

use crate::app::state::{PendingReviewCommentDraft, PendingReviewCommentSide, ReviewScreenState};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = ".critic";
const DRAFTS_DIR: &str = "drafts";
const DRAFT_FORMAT_VERSION: u8 = 1;

/// Disk-backed draft storage rooted at `~/.critic/drafts`.
#[derive(Debug, Clone)]
pub struct DraftStore {
    root: PathBuf,
}

/// Outcome of trying to restore a draft for a pull request.
#[derive(Debug, Clone)]
pub enum LoadOutcome {
    None,
    Loaded {
        pending_comments: Vec<PendingReviewCommentDraft>,
        reply_drafts: HashMap<String, String>,
        saved_head_sha: Option<String>,
    },
}

impl DraftStore {
    pub fn new() -> Result<Self> {
        let home =
            env::var_os("HOME").ok_or_else(|| anyhow!("HOME environment variable is not set"))?;
        let root = PathBuf::from(home).join(CONFIG_DIR).join(DRAFTS_DIR);
        fs::create_dir_all(&root)
            .with_context(|| format!("failed to create draft directory {}", root.display()))?;
        Ok(Self { root })
    }

    pub fn load_for_review(&self, review: &ReviewScreenState) -> Result<LoadOutcome> {
        let path = self.file_path_for_review(review);
        if !path.exists() {
            return Ok(LoadOutcome::None);
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read draft file {}", path.display()))?;
        let persisted: PersistedReviewDraft = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse draft file {}", path.display()))?;

        if persisted.version != DRAFT_FORMAT_VERSION {
            fs::remove_file(&path).with_context(|| {
                format!("failed to delete outdated draft file {}", path.display())
            })?;
            return Ok(LoadOutcome::None);
        }

        let saved_head_sha =
            (persisted.head_sha != review.pull.head_sha).then_some(persisted.head_sha.clone());

        Ok(LoadOutcome::Loaded {
            pending_comments: persisted
                .pending_review_comments
                .into_iter()
                .map(PendingReviewCommentDraft::from)
                .collect(),
            reply_drafts: persisted.reply_drafts,
            saved_head_sha,
        })
    }

    pub fn save_for_review(&self, review: &ReviewScreenState) -> Result<()> {
        let pending_review_comments = review
            .pending_review_comments()
            .iter()
            .cloned()
            .map(PersistedPendingReviewComment::from)
            .collect::<Vec<_>>();
        let reply_drafts = review.reply_drafts.clone();

        let persisted = PersistedReviewDraft {
            version: DRAFT_FORMAT_VERSION,
            owner: review.pull.owner.clone(),
            repo: review.pull.repo.clone(),
            pull_number: review.pull.number,
            head_sha: review.pull.head_sha.clone(),
            pending_review_comments,
            reply_drafts,
        };

        let path = self.file_path_for_review(review);
        let content =
            serde_json::to_string_pretty(&persisted).context("failed to serialize draft file")?;
        fs::write(&path, content)
            .with_context(|| format!("failed to write draft file {}", path.display()))?;
        Ok(())
    }

    pub fn clear_for_review(&self, review: &ReviewScreenState) -> Result<()> {
        let path = self.file_path_for_review(review);
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(&path)
            .with_context(|| format!("failed to delete draft file {}", path.display()))?;
        Ok(())
    }

    pub fn draft_signature(review: &ReviewScreenState) -> String {
        let mut signature = format!(
            "{}/{}/{}@{}|{}|",
            review.pull.owner,
            review.pull.repo,
            review.pull.number,
            review.pull.head_sha,
            review.pending_review_comments().len()
        );
        for comment in review.pending_review_comments() {
            signature.push_str(&format!(
                "{}:{}:{:?}:{}:{:?}:{}|",
                comment.id,
                comment.path,
                comment.side,
                comment.line,
                comment.start_line,
                comment.body
            ));
        }
        let mut reply_entries = review.reply_drafts.iter().collect::<Vec<_>>();
        reply_entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (key, value) in reply_entries {
            signature.push_str(&format!("{key}:{value}|"));
        }
        signature
    }

    fn file_path_for_review(&self, review: &ReviewScreenState) -> PathBuf {
        self.root.join(format!(
            "{}__{}__{}.json",
            sanitize_path_fragment(&review.pull.owner),
            sanitize_path_fragment(&review.pull.repo),
            review.pull.number
        ))
    }
}

fn sanitize_path_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedReviewDraft {
    version: u8,
    owner: String,
    repo: String,
    pull_number: u64,
    head_sha: String,
    pending_review_comments: Vec<PersistedPendingReviewComment>,
    #[serde(default)]
    reply_drafts: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPendingReviewComment {
    id: u64,
    path: String,
    side: PersistedPendingSide,
    line: u64,
    start_line: Option<u64>,
    body: String,
}

impl From<PendingReviewCommentDraft> for PersistedPendingReviewComment {
    fn from(value: PendingReviewCommentDraft) -> Self {
        Self {
            id: value.id,
            path: value.path,
            side: PersistedPendingSide::from(value.side),
            line: value.line,
            start_line: value.start_line,
            body: value.body,
        }
    }
}

impl From<PersistedPendingReviewComment> for PendingReviewCommentDraft {
    fn from(value: PersistedPendingReviewComment) -> Self {
        Self {
            id: value.id,
            path: value.path,
            side: PendingReviewCommentSide::from(value.side),
            line: value.line,
            start_line: value.start_line,
            body: value.body,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedPendingSide {
    Left,
    Right,
}

impl From<PendingReviewCommentSide> for PersistedPendingSide {
    fn from(value: PendingReviewCommentSide) -> Self {
        match value {
            PendingReviewCommentSide::Left => Self::Left,
            PendingReviewCommentSide::Right => Self::Right,
        }
    }
}

impl From<PersistedPendingSide> for PendingReviewCommentSide {
    fn from(value: PersistedPendingSide) -> Self {
        match value {
            PersistedPendingSide::Left => Self::Left,
            PersistedPendingSide::Right => Self::Right,
        }
    }
}
