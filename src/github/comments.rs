//! Pull request comment fetch and thread organization.

use crate::domain::{
    IssueComment, PullRequestComment, PullRequestData, PullRequestSummary, PullReviewSummary,
    ReviewComment, ReviewThread,
};
use octocrab::models::{CommentId, pulls};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Result type for pull request comment loading.
pub type Result<T> = std::result::Result<T, PullRequestCommentsError>;

/// Errors for pull request comment loading and transformation.
#[derive(Debug, Error)]
pub enum PullRequestCommentsError {
    #[error("GitHub API request failed: {0}")]
    Octocrab(#[from] octocrab::Error),
    #[error("graphql response error: {0}")]
    GraphQlResponseError(String),
    #[error("unsupported review event: {0}")]
    InvalidReviewEvent(String),
    #[error("review event {event} produced unexpected state {state}")]
    UnexpectedReviewState { event: String, state: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlCommentNode {
    database_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GraphQlReviewComments {
    nodes: Vec<GraphQlCommentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlReviewThreadNode {
    id: String,
    is_resolved: bool,
    comments: GraphQlReviewComments,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlReviewThreads {
    nodes: Vec<GraphQlReviewThreadNode>,
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPullRequest {
    review_threads: GraphQlReviewThreads,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlRepository {
    pull_request: Option<GraphQlPullRequest>,
}

#[derive(Debug, Deserialize)]
struct GraphQlData {
    repository: Option<GraphQlRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    data: Option<GraphQlData>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

const REVIEW_THREADS_RESOLUTION_QUERY: &str = r#"
query PullRequestReviewThreadResolution($owner: String!, $repo: String!, $pullNumber: Int!, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $pullNumber) {
      reviewThreads(first: 100, after: $after) {
        pageInfo {
          hasNextPage
          endCursor
        }
        nodes {
          id
          isResolved
          comments(first: 100) {
            nodes {
              databaseId
            }
          }
        }
      }
    }
  }
}
"#;

#[derive(Debug)]
struct BuildNode {
    comment: ReviewComment,
    replies: Vec<u64>,
}

/// Fetches all comment data for a pull request and returns merged review/issue entries.
pub async fn fetch_pull_request_data(
    client: &octocrab::Octocrab,
    pull: &PullRequestSummary,
) -> Result<PullRequestData> {
    let mut changed_files: Vec<String> =
        pull_request_file_paths(client, &pull.owner, &pull.repo, pull.number)
            .await?
            .into_iter()
            .collect();
    changed_files.sort();

    let review_threads =
        list_review_comment_threads(client, &pull.owner, &pull.repo, pull.number).await?;
    let issue_comments = list_issue_comments(client, &pull.owner, &pull.repo, pull.number).await?;
    let review_summaries =
        list_pull_review_summary_comments(client, &pull.owner, &pull.repo, pull.number).await?;

    let mut merged: Vec<(i64, PullRequestComment)> = review_threads
        .into_iter()
        .map(|thread| {
            (
                thread.comment.created_at.timestamp_millis(),
                PullRequestComment::ReviewThread(Box::new(thread)),
            )
        })
        .chain(issue_comments.into_iter().map(|comment| {
            (
                comment.created_at.timestamp_millis(),
                PullRequestComment::IssueComment(Box::new(comment)),
            )
        }))
        .chain(review_summaries.into_iter().map(|review| {
            (
                review
                    .submitted_at
                    .map(|value| value.timestamp_millis())
                    .unwrap_or_default(),
                PullRequestComment::ReviewSummary(Box::new(review)),
            )
        }))
        .collect();

    merged.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(PullRequestData {
        owner: pull.owner.clone(),
        repo: pull.repo.clone(),
        pull_number: pull.number,
        head_ref: pull.head_ref.clone(),
        base_ref: pull.base_ref.clone(),
        head_sha: pull.head_sha.clone(),
        base_sha: pull.base_sha.clone(),
        changed_files,
        comments: merged.into_iter().map(|(_, entry)| entry).collect(),
    })
}

/// Replies to an existing review comment.
pub async fn reply_to_review_comment(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
    comment_id: u64,
    body: &str,
) -> Result<()> {
    client
        .pulls(owner, repo)
        .reply_to_comment(pull_number, CommentId(comment_id), body.to_owned())
        .await?;

    Ok(())
}

/// Resolves or unresolves a review thread by GraphQL thread id.
pub async fn set_review_thread_resolved(
    client: &octocrab::Octocrab,
    thread_id: &str,
    resolved: bool,
) -> Result<()> {
    let query = if resolved {
        r#"
mutation ResolveReviewThread($threadId: ID!) {
  resolveReviewThread(input: {threadId: $threadId}) {
    thread { id }
  }
}
"#
    } else {
        r#"
mutation UnresolveReviewThread($threadId: ID!) {
  unresolveReviewThread(input: {threadId: $threadId}) {
    thread { id }
  }
}
"#
    };

    let response: serde_json::Value = client
        .graphql(&serde_json::json!({
            "query": query,
            "variables": {
                "threadId": thread_id,
            }
        }))
        .await?;

    if let Some(errors) = response.get("errors").and_then(|value| value.as_array()) {
        if !errors.is_empty() {
            let message = errors
                .iter()
                .filter_map(|value| value.get("message").and_then(|message| message.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(PullRequestCommentsError::GraphQlResponseError(message));
        }
    }

    Ok(())
}

/// Submits a pull request review with `COMMENT`, `APPROVE`, or `REQUEST_CHANGES`.
pub async fn submit_pull_request_review(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
    event: &str,
    body: &str,
) -> Result<()> {
    let event = match event {
        "COMMENT" | "APPROVE" | "REQUEST_CHANGES" => event,
        other => {
            return Err(PullRequestCommentsError::InvalidReviewEvent(
                other.to_owned(),
            ));
        }
    };

    let pull = client.pulls(owner, repo).get(pull_number).await?;
    let route = format!("/repos/{owner}/{repo}/pulls/{pull_number}/reviews");
    let review: pulls::Review = client
        .post(
            route,
            Some(&serde_json::json!({
                "body": body,
                "event": event,
                "commit_id": pull.head.sha,
            })),
        )
        .await?;

    let expected_state = match event {
        "COMMENT" => Some(pulls::ReviewState::Commented),
        "APPROVE" => Some(pulls::ReviewState::Approved),
        "REQUEST_CHANGES" => Some(pulls::ReviewState::ChangesRequested),
        _ => None,
    };

    if let Some(expected) = expected_state {
        if review.state != Some(expected) {
            return Err(PullRequestCommentsError::UnexpectedReviewState {
                event: event.to_owned(),
                state: format!("{:?}", review.state),
            });
        }
    }

    Ok(())
}

async fn list_review_comment_threads(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<Vec<ReviewThread>> {
    let first_page = client
        .pulls(owner, repo)
        .list_comments(Some(pull_number))
        .per_page(100)
        .send()
        .await?;
    let mapped = client.all_pages(first_page).await?;

    let resolved_by_comment_id =
        review_thread_resolution_map(client, owner, repo, pull_number).await?;

    let mut threads = build_review_threads(mapped);
    for thread in &mut threads {
        apply_thread_resolution(thread, &resolved_by_comment_id);
        if thread.thread_id.is_none() {
            set_thread_resolved_without_id(thread);
        }
    }

    Ok(threads)
}

async fn list_issue_comments(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<Vec<IssueComment>> {
    let first_page = client
        .issues(owner, repo)
        .list_comments(pull_number)
        .per_page(100)
        .send()
        .await?;
    client.all_pages(first_page).await.map_err(Into::into)
}

async fn list_pull_review_summary_comments(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<Vec<PullReviewSummary>> {
    let first_page = client
        .pulls(owner, repo)
        .list_reviews(pull_number)
        .per_page(100)
        .send()
        .await?;
    let reviews = client.all_pages(first_page).await?;

    Ok(reviews
        .into_iter()
        .filter(|review| {
            review
                .body
                .as_deref()
                .is_some_and(|body| !body.trim().is_empty())
        })
        .collect())
}

fn build_review_threads(comments: Vec<ReviewComment>) -> Vec<ReviewThread> {
    let mut nodes: HashMap<u64, BuildNode> = HashMap::with_capacity(comments.len());
    let mut parent_links: Vec<(u64, Option<u64>)> = Vec::with_capacity(comments.len());

    for comment in comments {
        let id = comment.id.into_inner();
        let parent_id = comment.in_reply_to_id.map(|parent| parent.into_inner());
        nodes.insert(
            id,
            BuildNode {
                comment,
                replies: Vec::new(),
            },
        );
        parent_links.push((id, parent_id));
    }

    let mut root_ids = Vec::new();
    for (id, parent_id) in parent_links {
        if let Some(parent_id) = parent_id {
            if let Some(parent) = nodes.get_mut(&parent_id) {
                parent.replies.push(id);
            } else {
                root_ids.push(id);
            }
        } else {
            root_ids.push(id);
        }
    }

    root_ids
        .into_iter()
        .filter_map(|id| materialize_thread(id, &mut nodes))
        .collect()
}

fn materialize_thread(id: u64, nodes: &mut HashMap<u64, BuildNode>) -> Option<ReviewThread> {
    let node = nodes.remove(&id)?;
    let replies = node
        .replies
        .iter()
        .copied()
        .filter_map(|reply_id| materialize_thread(reply_id, nodes))
        .collect();

    Some(ReviewThread {
        thread_id: None,
        is_resolved: false,
        comment: node.comment,
        replies,
    })
}

fn apply_thread_resolution(
    thread: &mut ReviewThread,
    resolved_by_comment_id: &HashMap<u64, (bool, String)>,
) {
    if let Some((is_resolved, thread_id)) = find_thread_resolution(thread, resolved_by_comment_id) {
        set_thread_resolution(thread, is_resolved, &thread_id);
    }
}

fn find_thread_resolution(
    thread: &ReviewThread,
    resolved_by_comment_id: &HashMap<u64, (bool, String)>,
) -> Option<(bool, String)> {
    if let Some((is_resolved, thread_id)) =
        resolved_by_comment_id.get(&thread.comment.id.into_inner())
    {
        return Some((*is_resolved, thread_id.clone()));
    }

    for reply in &thread.replies {
        if let Some(found) = find_thread_resolution(reply, resolved_by_comment_id) {
            return Some(found);
        }
    }

    None
}

fn set_thread_resolution(thread: &mut ReviewThread, is_resolved: bool, thread_id: &str) {
    thread.is_resolved = is_resolved;
    thread.thread_id = Some(thread_id.to_owned());
    for reply in &mut thread.replies {
        set_thread_resolution(reply, is_resolved, thread_id);
    }
}

fn set_thread_resolved_without_id(thread: &mut ReviewThread) {
    thread.is_resolved = true;
    for reply in &mut thread.replies {
        set_thread_resolved_without_id(reply);
    }
}

async fn review_thread_resolution_map(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<HashMap<u64, (bool, String)>> {
    let mut after: Option<String> = None;
    let mut resolved_by_comment_id = HashMap::new();

    loop {
        let response: GraphQlResponse = client
            .graphql(&serde_json::json!({
                "query": REVIEW_THREADS_RESOLUTION_QUERY,
                "variables": {
                    "owner": owner,
                    "repo": repo,
                    "pullNumber": pull_number,
                    "after": after,
                }
            }))
            .await?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                let message = errors
                    .into_iter()
                    .map(|error| error.message)
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(PullRequestCommentsError::GraphQlResponseError(message));
            }
        }

        let Some(review_threads) = response
            .data
            .and_then(|data| data.repository)
            .and_then(|repository| repository.pull_request)
            .map(|pull_request| pull_request.review_threads)
        else {
            return Err(PullRequestCommentsError::GraphQlResponseError(
                "missing review thread data in GraphQL response".to_owned(),
            ));
        };

        for thread in &review_threads.nodes {
            for comment in &thread.comments.nodes {
                if let Some(comment_id) = comment.database_id {
                    resolved_by_comment_id
                        .insert(comment_id, (thread.is_resolved, thread.id.clone()));
                }
            }
        }

        if !review_threads.page_info.has_next_page {
            break;
        }

        after = review_threads.page_info.end_cursor;
    }

    Ok(resolved_by_comment_id)
}

async fn pull_request_file_paths(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> Result<HashSet<String>> {
    let first_page = client.pulls(owner, repo).list_files(pull_number).await?;
    let files = client.all_pages(first_page).await?;

    Ok(files.into_iter().map(|file| file.filename).collect())
}

#[cfg(test)]
mod tests {
    use super::{ReviewComment, build_review_threads};
    use serde_json::json;

    fn review_comment(id: u64, in_reply_to_id: Option<u64>) -> ReviewComment {
        let mut payload = json!({
            "url": format!("https://example.invalid/comments/{id}"),
            "id": id,
            "node_id": format!("PRRC_{id}"),
            "diff_hunk": "@@ -1,1 +1,1 @@",
            "path": "src/lib.rs",
            "commit_id": "deadbeef",
            "original_commit_id": "deadbeef",
            "body": format!("comment {id}"),
            "created_at": "2026-02-01T00:00:00Z",
            "updated_at": "2026-02-01T00:00:00Z",
            "html_url": format!("https://example.invalid/comments/{id}"),
            "_links": {},
            "line": 1
        });

        if let Some(reply_to) = in_reply_to_id {
            payload["in_reply_to_id"] = json!(reply_to);
        }

        serde_json::from_value(payload).expect("valid pull review comment fixture")
    }

    #[test]
    fn builds_reply_under_parent() {
        let comments = vec![review_comment(1, None), review_comment(2, Some(1))];
        let threads = build_review_threads(comments);

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].comment.id.into_inner(), 1);
        assert_eq!(threads[0].replies.len(), 1);
        assert_eq!(threads[0].replies[0].comment.id.into_inner(), 2);
    }

    #[test]
    fn orphan_reply_becomes_root() {
        let comments = vec![review_comment(5, Some(999))];
        let threads = build_review_threads(comments);

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].comment.id.into_inner(), 5);
    }
}
