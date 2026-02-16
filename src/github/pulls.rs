//! Open pull request discovery and mapping for the search screen.

use crate::domain::{PullRequestReviewStatus, PullRequestSummary};
use octocrab::models::pulls::ReviewState;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use thiserror::Error;
use tokio::process::Command;
use tokio::task::JoinSet;

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
    #[error("GitHub API request failed: {0}")]
    Octocrab(#[from] octocrab::Error),
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
    let review_statuses = fetch_review_statuses(client, repository, &pulls).await;

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
            let head = pull.head;
            let base = pull.base;
            let updated = pull.updated_at.or(pull.created_at);
            let updated_at = updated
                .map(|time| time.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_owned());
            let updated_ms = updated
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
                updated_at,
                updated_at_unix_ms: updated_ms,
                review_status: review_statuses.get(&pull.number).copied().flatten(),
            }
        })
        .collect();

    Ok(mapped)
}

async fn fetch_review_statuses(
    client: &octocrab::Octocrab,
    repository: &RepositoryRef,
    pulls: &[octocrab::models::pulls::PullRequest],
) -> HashMap<u64, Option<PullRequestReviewStatus>> {
    let mut pending: VecDeque<u64> = pulls.iter().map(|pull| pull.number).collect();
    let mut out = HashMap::with_capacity(pulls.len());
    let mut in_flight = JoinSet::new();
    let concurrency = pending.len().clamp(1, 8);

    for _ in 0..concurrency {
        spawn_review_status_task(&mut in_flight, client, repository, &mut pending);
    }

    while let Some(joined) = in_flight.join_next().await {
        if let Ok((number, status)) = joined {
            out.insert(number, status);
        }
        spawn_review_status_task(&mut in_flight, client, repository, &mut pending);
    }

    out
}

fn spawn_review_status_task(
    in_flight: &mut JoinSet<(u64, Option<PullRequestReviewStatus>)>,
    client: &octocrab::Octocrab,
    repository: &RepositoryRef,
    pending: &mut VecDeque<u64>,
) {
    let Some(number) = pending.pop_front() else {
        return;
    };

    let client = client.clone();
    let owner = repository.owner.clone();
    let repo = repository.repo.clone();

    in_flight.spawn(async move {
        let status = fetch_pull_review_status(&client, &owner, &repo, number)
            .await
            .unwrap_or(None);
        (number, status)
    });
}

async fn fetch_pull_review_status(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    pull_number: u64,
) -> std::result::Result<Option<PullRequestReviewStatus>, octocrab::Error> {
    let first_page = client
        .pulls(owner, repo)
        .list_reviews(pull_number)
        .per_page(100)
        .send()
        .await?;
    let reviews = client.all_pages(first_page).await?;

    Ok(classify_review_status(&reviews))
}

fn classify_review_status(
    reviews: &[octocrab::models::pulls::Review],
) -> Option<PullRequestReviewStatus> {
    let mut latest_by_user: HashMap<&str, PullRequestReviewStatus> = HashMap::new();

    for review in reviews {
        let Some(user) = review.user.as_ref() else {
            continue;
        };
        let Some(state) = review.state else {
            continue;
        };

        match state {
            ReviewState::Approved => {
                latest_by_user.insert(user.login.as_str(), PullRequestReviewStatus::Approved);
            }
            ReviewState::ChangesRequested => {
                latest_by_user.insert(
                    user.login.as_str(),
                    PullRequestReviewStatus::ChangesRequested,
                );
            }
            ReviewState::Dismissed => {
                latest_by_user.remove(user.login.as_str());
            }
            ReviewState::Open | ReviewState::Pending | ReviewState::Commented => {}
            _ => {}
        }
    }

    if latest_by_user
        .values()
        .any(|state| *state == PullRequestReviewStatus::ChangesRequested)
    {
        return Some(PullRequestReviewStatus::ChangesRequested);
    }
    if latest_by_user
        .values()
        .any(|state| *state == PullRequestReviewStatus::Approved)
    {
        return Some(PullRequestReviewStatus::Approved);
    }

    None
}
