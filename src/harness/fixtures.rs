//! Deterministic fixture data used by the visual harness.

use crate::domain::{
    IssueComment, PullRequestComment, PullRequestData, PullRequestSummary, ReviewComment,
    ReviewThread,
};

/// Returns fixture pull requests for the search screen.
pub fn demo_pull_requests() -> Vec<PullRequestSummary> {
    vec![
        PullRequestSummary {
            owner: "demo-org".to_owned(),
            repo: "sample-repo".to_owned(),
            number: 1042,
            title: "Tighten parser state handling in message codec".to_owned(),
            author: "mock_author_01".to_owned(),
            head_ref: "feature/parser-state".to_owned(),
            base_ref: "main".to_owned(),
            html_url: Some("https://example.invalid/demo-org/sample-repo/pull/1042".to_owned()),
            updated_at: "2026-02-13T09:42:00Z".to_owned(),
            updated_at_unix_ms: 1_771_070_920_000,
        },
        PullRequestSummary {
            owner: "demo-org".to_owned(),
            repo: "sample-repo".to_owned(),
            number: 1037,
            title: "Refactor wire format validation checkpoints".to_owned(),
            author: "mock_author_02".to_owned(),
            head_ref: "feature/wire-checkpoints".to_owned(),
            base_ref: "main".to_owned(),
            html_url: Some("https://example.invalid/demo-org/sample-repo/pull/1037".to_owned()),
            updated_at: "2026-02-12T15:02:00Z".to_owned(),
            updated_at_unix_ms: 1_770_992_520_000,
        },
    ]
}

/// Returns fixture comment data for a selected demo PR.
pub fn demo_pull_request_data_for(pull: &PullRequestSummary) -> PullRequestData {
    let root = ReviewThread {
        thread_id: Some("PRRT_kwDOX-1234M4A9".to_owned()),
        is_resolved: false,
        comment: ReviewComment {
            id: 1001,
            in_reply_to_id: None,
            body: "nit: consider renaming `MessageEnvelope` to `FrameEnvelope`".to_owned(),
            diff_hunk: Some(
                "@@ -10,8 +10,11 @@\n pub type MessageId = u64;\n+/// Wraps transport metadata with decoded payload.\n pub struct MessageEnvelope {\n     pub id: MessageId,\n     pub body: Vec<u8>,\n }\n"
                    .to_owned(),
            ),
            path: Some("src/codec/message.rs".to_owned()),
            line: Some(14),
            start_line: None,
            original_line: Some(13),
            original_start_line: None,
            side: Some("RIGHT".to_owned()),
            html_url: Some(
                "https://example.invalid/demo-org/sample-repo/pull/1042#discussion_r1001"
                    .to_owned(),
            ),
            created_at: "2026-02-13T03:32:00Z".to_owned(),
            author: "mock_reviewer_01".to_owned(),
        },
        replies: vec![ReviewThread {
            thread_id: Some("PRRT_kwDOX-1234M4A9".to_owned()),
            is_resolved: false,
            comment: ReviewComment {
                id: 1002,
                in_reply_to_id: Some(1001),
                body: "Agreed. `FrameEnvelope` makes call sites easier to read and clarifies payload ownership."
                    .to_owned(),
                diff_hunk: None,
                path: Some("src/codec/message.rs".to_owned()),
                line: Some(14),
                start_line: None,
                original_line: Some(13),
                original_start_line: None,
                side: Some("RIGHT".to_owned()),
                html_url: Some(
                    "https://example.invalid/demo-org/sample-repo/pull/1042#discussion_r1002"
                        .to_owned(),
                ),
                created_at: "2026-02-13T09:42:00Z".to_owned(),
                author: "mock_author_01".to_owned(),
            },
            replies: vec![],
        }],
    };

    let outdated = ReviewThread {
        thread_id: Some("PRRT_kwDOX-1234M4B2".to_owned()),
        is_resolved: false,
        comment: ReviewComment {
            id: 1003,
            in_reply_to_id: None,
            body: "This thread references an outdated diff context.".to_owned(),
            diff_hunk: None,
            path: None,
            line: None,
            start_line: None,
            original_line: Some(42),
            original_start_line: None,
            side: Some("RIGHT".to_owned()),
            html_url: Some(
                "https://example.invalid/demo-org/sample-repo/pull/1042#discussion_r1003"
                    .to_owned(),
            ),
            created_at: "2026-02-13T11:05:00Z".to_owned(),
            author: "mock_reviewer_02".to_owned(),
        },
        replies: vec![],
    };

    PullRequestData {
        owner: pull.owner.clone(),
        repo: pull.repo.clone(),
        pull_number: pull.number,
        head_ref: pull.head_ref.clone(),
        base_ref: pull.base_ref.clone(),
        changed_files: vec![
            "src/codec/message.rs".to_owned(),
            "src/codec/mod.rs".to_owned(),
        ],
        comments: vec![
            PullRequestComment::ReviewThread(Box::new(root)),
            PullRequestComment::ReviewThread(Box::new(outdated)),
            PullRequestComment::IssueComment(Box::new(IssueComment {
                id: 7001,
                body: "Looks good overall; I just left one small naming suggestion.".to_owned(),
                html_url: Some(
                    "https://example.invalid/demo-org/sample-repo/pull/1042#issuecomment_7001"
                        .to_owned(),
                ),
                created_at: "2026-02-13T10:01:00Z".to_owned(),
                author: "mock_maintainer".to_owned(),
            })),
        ],
    }
}
