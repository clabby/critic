//! Deterministic fixture data used by the visual harness.

use crate::domain::{
    IssueComment, PullRequestComment, PullRequestData, PullRequestReviewStatus, PullRequestSummary,
    PullReviewSummary, ReviewComment, ReviewThread,
};
use serde_json::json;

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
            head_sha: "1111111111111111111111111111111111111111".to_owned(),
            base_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            html_url: Some("https://example.invalid/demo-org/sample-repo/pull/1042".to_owned()),
            updated_at: "2026-02-13T09:42:00Z".to_owned(),
            updated_at_unix_ms: 1_771_070_920_000,
            review_status: Some(PullRequestReviewStatus::Approved),
        },
        PullRequestSummary {
            owner: "demo-org".to_owned(),
            repo: "sample-repo".to_owned(),
            number: 1037,
            title: "Refactor wire format validation checkpoints".to_owned(),
            author: "mock_author_02".to_owned(),
            head_ref: "feature/wire-checkpoints".to_owned(),
            base_ref: "main".to_owned(),
            head_sha: "2222222222222222222222222222222222222222".to_owned(),
            base_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            html_url: Some("https://example.invalid/demo-org/sample-repo/pull/1037".to_owned()),
            updated_at: "2026-02-12T15:02:00Z".to_owned(),
            updated_at_unix_ms: 1_770_992_520_000,
            review_status: Some(PullRequestReviewStatus::ChangesRequested),
        },
    ]
}

/// Returns fixture comment data for a selected demo PR.
pub fn demo_pull_request_data_for(pull: &PullRequestSummary) -> PullRequestData {
    let grouped_root = ReviewThread {
        thread_id: Some("PRRT_kwDOX-1234M4A9".to_owned()),
        is_resolved: false,
        comment: review_comment_fixture(ReviewCommentFixture {
            id: 1001,
            in_reply_to_id: None,
            pull_request_review_id: Some(8101),
            author: "mock_reviewer_01",
            body: "Test thread #1",
            diff_hunk: "@@ -10,8 +10,11 @@\n pub type MessageId = u64;\n+/// Wraps transport metadata with decoded payload.\n pub struct MessageEnvelope {\n     pub id: MessageId,\n     pub body: Vec<u8>,\n }\n",
            path: "src/app/editor.rs",
            line: Some(14),
        }),
        replies: vec![ReviewThread {
            thread_id: Some("PRRT_kwDOX-1234M4A9".to_owned()),
            is_resolved: false,
            comment: review_comment_fixture(ReviewCommentFixture {
                id: 1002,
                in_reply_to_id: Some(1001),
                pull_request_review_id: Some(8101),
                author: "mock_author_01",
                body: "Following up on the editor changes.",
                diff_hunk: "",
                path: "src/app/editor.rs",
                line: Some(14),
            }),
            replies: vec![],
        }],
    };

    let grouped_resolved = ReviewThread {
        thread_id: Some("PRRT_kwDOX-1234M4B2".to_owned()),
        is_resolved: true,
        comment: review_comment_fixture(ReviewCommentFixture {
            id: 1003,
            in_reply_to_id: None,
            pull_request_review_id: Some(8101),
            author: "mock_reviewer_02",
            body: "Resolved thread under the same review summary.",
            diff_hunk: "",
            path: "src/app/editor.rs",
            line: Some(22),
        }),
        replies: vec![],
    };

    let standalone = ReviewThread {
        thread_id: Some("PRRT_kwDOX-1234M4C7".to_owned()),
        is_resolved: false,
        comment: review_comment_fixture(ReviewCommentFixture {
            id: 1004,
            in_reply_to_id: None,
            pull_request_review_id: None,
            author: "mock_reviewer_04",
            body: "Standalone review thread not linked to a review summary.",
            diff_hunk: "@@ -31,2 +33,3 @@\n+ println!(\"debug\");\n",
            path: "src/codec/mod.rs",
            line: Some(33),
        }),
        replies: vec![],
    };

    PullRequestData {
        owner: pull.owner.clone(),
        repo: pull.repo.clone(),
        pull_number: pull.number,
        head_ref: pull.head_ref.clone(),
        base_ref: pull.base_ref.clone(),
        head_sha: pull.head_sha.clone(),
        base_sha: pull.base_sha.clone(),
        changed_files: vec![
            "src/app/editor.rs".to_owned(),
            "src/codec/mod.rs".to_owned(),
        ],
        comments: vec![
            PullRequestComment::ReviewSummary(Box::new(review_summary_fixture(
                8101,
                "mock_reviewer_03",
                "Test meta comment",
            ))),
            PullRequestComment::ReviewThread(Box::new(grouped_root)),
            PullRequestComment::ReviewThread(Box::new(grouped_resolved)),
            PullRequestComment::IssueComment(Box::new(issue_comment_fixture(
                7001,
                "mock_maintainer",
                "Looks good overall; I just left one small naming suggestion.",
            ))),
            PullRequestComment::ReviewSummary(Box::new(review_summary_fixture(
                8102,
                "mock_reviewer_05",
                "Standalone top-level review comment (no sub-threads).",
            ))),
            PullRequestComment::ReviewThread(Box::new(standalone)),
            PullRequestComment::ReviewSummary(Box::new(review_summary_fixture(
                8103,
                "mock_reviewer_06",
                "Another standalone review summary.",
            ))),
        ],
    }
}

struct ReviewCommentFixture<'a> {
    id: u64,
    in_reply_to_id: Option<u64>,
    pull_request_review_id: Option<u64>,
    author: &'a str,
    body: &'a str,
    diff_hunk: &'a str,
    path: &'a str,
    line: Option<u64>,
}

fn review_comment_fixture(input: ReviewCommentFixture<'_>) -> ReviewComment {
    let ReviewCommentFixture {
        id,
        in_reply_to_id,
        pull_request_review_id,
        author,
        body,
        diff_hunk,
        path,
        line,
    } = input;

    let mut payload = json!({
        "url": format!("https://example.invalid/comments/{id}"),
        "id": id,
        "node_id": format!("PRRC_{id}"),
        "diff_hunk": diff_hunk,
        "path": path,
        "commit_id": "deadbeef",
        "original_commit_id": "deadbeef",
        "user": author_json(author, id + 10),
        "body": body,
        "created_at": "2026-02-13T03:32:00Z",
        "updated_at": "2026-02-13T03:32:00Z",
        "html_url": format!("https://example.invalid/comments/{id}"),
        "_links": {},
        "line": line,
        "side": "RIGHT"
    });

    if let Some(reply_to) = in_reply_to_id {
        payload["in_reply_to_id"] = json!(reply_to);
    }
    if let Some(review_id) = pull_request_review_id {
        payload["pull_request_review_id"] = json!(review_id);
    }

    serde_json::from_value(payload).expect("valid review comment fixture")
}

fn issue_comment_fixture(id: u64, author: &str, body: &str) -> IssueComment {
    serde_json::from_value(json!({
        "id": id,
        "node_id": format!("IC_{id}"),
        "url": format!("https://example.invalid/issue-comments/{id}"),
        "html_url": format!("https://example.invalid/issue-comments/{id}"),
        "body": body,
        "user": author_json(author, id + 20),
        "created_at": "2026-02-13T10:01:00Z"
    }))
    .expect("valid issue comment fixture")
}

fn review_summary_fixture(id: u64, author: &str, body: &str) -> PullReviewSummary {
    serde_json::from_value(json!({
        "id": id,
        "node_id": format!("PRR_{id}"),
        "html_url": format!("https://example.invalid/reviews/{id}"),
        "user": author_json(author, id + 30),
        "body": body,
        "state": "APPROVED",
        "submitted_at": "2026-02-13T11:09:00Z"
    }))
    .expect("valid review summary fixture")
}

fn author_json(login: &str, id: u64) -> serde_json::Value {
    json!({
        "login": login,
        "id": id,
        "node_id": format!("U_{id}"),
        "avatar_url": "https://example.invalid/avatar.png",
        "gravatar_id": "",
        "url": "https://example.invalid/user",
        "html_url": "https://example.invalid/user",
        "followers_url": "https://example.invalid/followers",
        "following_url": "https://example.invalid/following{/other_user}",
        "gists_url": "https://example.invalid/gists{/gist_id}",
        "starred_url": "https://example.invalid/starred{/owner}{/repo}",
        "subscriptions_url": "https://example.invalid/subscriptions",
        "organizations_url": "https://example.invalid/orgs",
        "repos_url": "https://example.invalid/repos",
        "events_url": "https://example.invalid/events{/privacy}",
        "received_events_url": "https://example.invalid/received_events",
        "type": "User",
        "site_admin": false
    })
}
