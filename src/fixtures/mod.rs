//! Deterministic fixture data for demo mode and harness rendering.

use crate::domain::{
    IssueComment, PullRequestComment, PullRequestData, PullRequestSummary, ReviewComment,
    ReviewThread,
};

/// Returns fixture pull requests for the search screen.
pub fn demo_pull_requests() -> Vec<PullRequestSummary> {
    vec![
        PullRequestSummary {
            owner: "commonwarexyz".to_owned(),
            repo: "monorepo".to_owned(),
            number: 2208,
            title: "Improve digest handling in consensus cert validation".to_owned(),
            author: "clabby".to_owned(),
            head_ref: "feature/cert-digest".to_owned(),
            base_ref: "main".to_owned(),
            html_url: Some("https://github.com/commonwarexyz/monorepo/pull/2208".to_owned()),
            updated_at: "2026-02-13T09:42:00Z".to_owned(),
            updated_at_unix_ms: 1_771_070_920_000,
        },
        PullRequestSummary {
            owner: "commonwarexyz".to_owned(),
            repo: "monorepo".to_owned(),
            number: 2204,
            title: "Refactor marshaling boundary checks".to_owned(),
            author: "patrick-ogrady".to_owned(),
            head_ref: "feature/marshal-boundary".to_owned(),
            base_ref: "main".to_owned(),
            html_url: Some("https://github.com/commonwarexyz/monorepo/pull/2204".to_owned()),
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
            body: "nit: `Certificate`".to_owned(),
            diff_hunk: Some(
                "@@ -1,17 +1,20 @@\n type BlockDigest: Digest;\n+/// The type of `Digest` included in consensus certificates.\n type Commitment: Digest;"
                    .to_owned(),
            ),
            path: Some("consensus/src/marshal/store.rs".to_owned()),
            line: Some(18),
            start_line: None,
            original_line: Some(17),
            original_start_line: None,
            side: Some("RIGHT".to_owned()),
            html_url: Some(
                "https://github.com/commonwarexyz/monorepo/pull/2208#discussion_r2804568679"
                    .to_owned(),
            ),
            created_at: "2026-02-13T03:32:00Z".to_owned(),
            author: "patrick-ogrady".to_owned(),
        },
        replies: vec![ReviewThread {
            thread_id: Some("PRRT_kwDOX-1234M4A9".to_owned()),
            is_resolved: false,
            comment: ReviewComment {
                id: 1002,
                in_reply_to_id: Some(1001),
                body: "I think it's a bit more clear how it is now - if I saw `Certificate` I'd want to put the cert type there, not the cert's payload.".to_owned(),
                diff_hunk: None,
                path: Some("consensus/src/marshal/store.rs".to_owned()),
                line: Some(18),
                start_line: None,
                original_line: Some(17),
                original_start_line: None,
                side: Some("RIGHT".to_owned()),
                html_url: Some(
                    "https://github.com/commonwarexyz/monorepo/pull/2208#discussion_r2804600012"
                        .to_owned(),
                ),
                created_at: "2026-02-13T09:42:00Z".to_owned(),
                author: "clabby".to_owned(),
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
            body: "This comment is on an outdated diff context.".to_owned(),
            diff_hunk: None,
            path: None,
            line: None,
            start_line: None,
            original_line: Some(42),
            original_start_line: None,
            side: Some("RIGHT".to_owned()),
            html_url: Some(
                "https://github.com/commonwarexyz/monorepo/pull/2208#discussion_r2804666666"
                    .to_owned(),
            ),
            created_at: "2026-02-13T11:05:00Z".to_owned(),
            author: "patrick-ogrady".to_owned(),
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
            "consensus/src/marshal/store.rs".to_owned(),
            "consensus/src/certificate.rs".to_owned(),
        ],
        comments: vec![
            PullRequestComment::ReviewThread(Box::new(root)),
            PullRequestComment::ReviewThread(Box::new(outdated)),
            PullRequestComment::IssueComment(Box::new(IssueComment {
                id: 7001,
                body: "LGTM overall, one minor note.".to_owned(),
                html_url: Some(
                    "https://github.com/commonwarexyz/monorepo/pull/2208#issuecomment-3010101010"
                        .to_owned(),
                ),
                created_at: "2026-02-13T10:01:00Z".to_owned(),
                author: "clabby".to_owned(),
            })),
        ],
    }
}
