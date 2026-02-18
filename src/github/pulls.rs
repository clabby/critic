//! Open pull request discovery and mapping for the search screen.

use crate::{
    domain::{PullRequestReviewStatus, PullRequestSummary},
    github::errors::format_octocrab_error,
};
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;
use tokio::process::Command;

/// Result type for pull request queries.
pub type Result<T> = std::result::Result<T, PullRequestQueryError>;

/// Errors returned while resolving repo context and loading open PRs.
#[derive(Debug, Error)]
pub enum PullRequestQueryError {
    #[error("repository owner/repo must both be provided together")]
    PartialRepositoryArgs,
    #[error("failed to resolve repository from `gh repo view` ({0})")]
    GhRepoViewUnavailable(std::io::Error),
    #[error("`gh repo view` failed with status {status}: {stderr}")]
    GhRepoViewFailed { status: i32, stderr: String },
    #[error("failed to parse `gh repo view` output: {0}")]
    GhRepoViewInvalidJson(serde_json::Error),
    #[error("GitHub API request failed: {message}")]
    Octocrab {
        message: String,
        status_code: Option<u16>,
    },
}

impl From<octocrab::Error> for PullRequestQueryError {
    fn from(error: octocrab::Error) -> Self {
        let status_code = match &error {
            octocrab::Error::GitHub { source, .. } => Some(source.status_code.as_u16()),
            _ => None,
        };

        Self::Octocrab {
            message: format_octocrab_error(error),
            status_code,
        }
    }
}

impl PullRequestQueryError {
    /// Returns true when the query failed because the resource was not found.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::Octocrab {
                status_code: Some(404),
                ..
            }
        )
    }
}

/// Repository identity used for pull request listing.
#[derive(Debug, Clone)]
pub struct RepositoryRef {
    pub owner: String,
    pub repo: String,
}

impl RepositoryRef {
    pub fn label(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

#[derive(Debug, Deserialize)]
struct GhRepoViewOwner {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GhRepoViewPayload {
    name: String,
    owner: GhRepoViewOwner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GraphQlReviewDecision {
    Approved,
    ChangesRequested,
    ReviewRequired,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPullNode {
    number: u64,
    review_decision: Option<GraphQlReviewDecision>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphQlPullConnection {
    nodes: Vec<GraphQlPullNode>,
    page_info: GraphQlPageInfo,
}

#[derive(Debug, Deserialize)]
struct GraphQlRepository {
    #[serde(rename = "pullRequests")]
    pull_requests: GraphQlPullConnection,
}

#[derive(Debug, Deserialize)]
struct GraphQlData {
    repository: Option<GraphQlRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    data: Option<GraphQlData>,
    errors: Option<Vec<GraphQlError>>,
}

const OPEN_PULL_REVIEW_DECISIONS_QUERY: &str = r#"
query OpenPullReviewDecisions($owner: String!, $repo: String!, $after: String) {
  repository(owner: $owner, name: $repo) {
    pullRequests(
      first: 100,
      states: OPEN,
      after: $after,
      orderBy: {field: UPDATED_AT, direction: DESC}
    ) {
      pageInfo {
        hasNextPage
        endCursor
      }
      nodes {
        number
        reviewDecision
      }
    }
  }
}
"#;

/// Resolves repository context from explicit args, or `gh repo view` when omitted.
pub async fn resolve_repository(
    owner: Option<String>,
    repo: Option<String>,
) -> Result<RepositoryRef> {
    match (owner, repo) {
        (Some(owner), Some(repo)) => Ok(RepositoryRef { owner, repo }),
        (None, None) => resolve_repository_from_gh().await,
        _ => Err(PullRequestQueryError::PartialRepositoryArgs),
    }
}

async fn resolve_repository_from_gh() -> Result<RepositoryRef> {
    let output = Command::new("gh")
        .arg("repo")
        .arg("view")
        .arg("--json")
        .arg("name,owner")
        .output()
        .await
        .map_err(PullRequestQueryError::GhRepoViewUnavailable)?;

    if !output.status.success() {
        return Err(PullRequestQueryError::GhRepoViewFailed {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let payload: GhRepoViewPayload = serde_json::from_slice(&output.stdout)
        .map_err(PullRequestQueryError::GhRepoViewInvalidJson)?;

    Ok(RepositoryRef {
        owner: payload.owner.login,
        repo: payload.name,
    })
}

/// Fetches open pull requests for the target repository.
pub async fn fetch_open_pull_requests(
    client: &octocrab::Octocrab,
    repository: &RepositoryRef,
) -> Result<Vec<PullRequestSummary>> {
    use octocrab::params::State;

    let first_page = client
        .pulls(&repository.owner, &repository.repo)
        .list()
        .state(State::Open)
        .per_page(100)
        .send()
        .await?;

    let mut pulls = client.all_pages(first_page).await?;
    let review_statuses = fetch_review_statuses(client, repository).await;

    pulls.sort_by(|a, b| {
        let a_ts = a
            .updated_at
            .or(a.created_at)
            .map(|ts| ts.timestamp_millis())
            .unwrap_or_default();
        let b_ts = b
            .updated_at
            .or(b.created_at)
            .map(|ts| ts.timestamp_millis())
            .unwrap_or_default();
        b_ts.cmp(&a_ts)
    });

    let mapped = pulls
        .into_iter()
        .map(|pull| {
            let number = pull.number;
            map_pull_request(
                repository,
                pull,
                review_statuses.get(&number).copied().flatten(),
            )
        })
        .collect();

    Ok(mapped)
}

/// Fetches the authenticated viewer login.
pub async fn fetch_viewer_login(client: &octocrab::Octocrab) -> Result<String> {
    let user = client.current().user().await?;
    Ok(user.login)
}

/// Fetches a single pull request summary by number.
pub async fn fetch_pull_request_summary(
    client: &octocrab::Octocrab,
    repository: &RepositoryRef,
    pull_number: u64,
) -> Result<PullRequestSummary> {
    let pull = client
        .pulls(&repository.owner, &repository.repo)
        .get(pull_number)
        .await?;
    Ok(map_pull_request(repository, pull, None))
}

async fn fetch_review_statuses(
    client: &octocrab::Octocrab,
    repository: &RepositoryRef,
) -> HashMap<u64, Option<PullRequestReviewStatus>> {
    let mut after: Option<String> = None;
    let mut out = HashMap::new();

    loop {
        let response: std::result::Result<GraphQlResponse, octocrab::Error> = client
            .graphql(&serde_json::json!({
                "query": OPEN_PULL_REVIEW_DECISIONS_QUERY,
                "variables": {
                    "owner": &repository.owner,
                    "repo": &repository.repo,
                    "after": &after,
                }
            }))
            .await;

        let Ok(response) = response else {
            return out;
        };

        if response
            .errors
            .as_ref()
            .is_some_and(|errors| !errors.is_empty())
        {
            let _messages = response
                .errors
                .as_ref()
                .map(|errors| {
                    errors
                        .iter()
                        .map(|error| error.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            return out;
        }

        let Some(connection) = response
            .data
            .and_then(|data| data.repository)
            .map(|repo| repo.pull_requests)
        else {
            return out;
        };

        for pull in &connection.nodes {
            let status = match pull.review_decision {
                Some(GraphQlReviewDecision::Approved) => Some(PullRequestReviewStatus::Approved),
                Some(GraphQlReviewDecision::ChangesRequested) => {
                    Some(PullRequestReviewStatus::ChangesRequested)
                }
                Some(GraphQlReviewDecision::ReviewRequired) | None => None,
            };
            out.insert(pull.number, status);
        }

        if !connection.page_info.has_next_page {
            break;
        }
        after = connection.page_info.end_cursor;
    }

    out
}

fn map_pull_request(
    repository: &RepositoryRef,
    pull: octocrab::models::pulls::PullRequest,
    review_status: Option<PullRequestReviewStatus>,
) -> PullRequestSummary {
    let head = pull.head;
    let base = pull.base;
    let created = pull.created_at;
    let updated = pull.updated_at.or_else(|| created.clone());
    let updated_ms = updated
        .map(|time| time.timestamp_millis())
        .unwrap_or_default();
    let created_ms = created
        .map(|time| time.timestamp_millis())
        .unwrap_or_default();

    PullRequestSummary {
        owner: repository.owner.clone(),
        repo: repository.repo.clone(),
        number: pull.number,
        title: pull.title.unwrap_or_else(|| "(untitled)".to_owned()),
        author: pull
            .user
            .as_ref()
            .map(|user| user.login.clone())
            .unwrap_or_else(|| "unknown".to_owned()),
        head_ref: head.ref_field,
        base_ref: base.ref_field,
        head_sha: head.sha,
        base_sha: base.sha,
        html_url: pull.html_url.map(|url| url.to_string()),
        updated_at_unix_ms: updated_ms,
        created_at_unix_ms: created_ms,
        review_status,
    }
}
