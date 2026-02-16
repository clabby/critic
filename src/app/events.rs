//! Background worker messages and async data-loading tasks.

use crate::domain::{PullRequestData, PullRequestDiffData, PullRequestSummary};
use crate::github::comments::{
    SubmitPullRequestReviewRequest, SubmitReviewComment, fetch_pull_request_data,
    reply_to_review_comment, set_review_thread_resolved, submit_pull_request_review,
};
use crate::github::diff::fetch_pull_request_diff_data;
use crate::github::pulls::{
    fetch_open_pull_requests, fetch_pull_request_summary, resolve_repository,
};
use tokio::sync::mpsc::UnboundedSender;

/// Message sent from background workers to the UI event loop.
#[derive(Debug)]
pub enum WorkerMessage {
    PullRequestsLoaded {
        repository_label: String,
        result: Result<Vec<PullRequestSummary>, String>,
    },
    PullRequestResolved {
        repository_label: String,
        pull_number: u64,
        result: Result<PullRequestSummary, String>,
    },
    PullRequestDataLoaded {
        pull: PullRequestSummary,
        result: Result<PullRequestData, String>,
    },
    PullRequestDiffLoaded {
        pull: PullRequestSummary,
        result: Result<PullRequestDiffData, String>,
    },
    MutationApplied {
        pull: PullRequestSummary,
        clear_reply_root_key: Option<String>,
        clear_pending_review_comments: bool,
        result: Result<PullRequestData, String>,
    },
}

/// Mutation actions supported by the review screen.
#[derive(Debug, Clone)]
pub enum MutationRequest {
    ReplyToReviewComment {
        owner: String,
        repo: String,
        pull_number: u64,
        comment_id: u64,
        body: String,
    },
    SetReviewThreadResolved {
        thread_id: String,
        resolved: bool,
    },
    SubmitPullRequestReview {
        owner: String,
        repo: String,
        pull_number: u64,
        event: String,
        body: String,
        comments: Vec<SubmitReviewComment>,
        expected_head_sha: String,
    },
}

/// Spawns async loading of the open pull request list.
pub fn spawn_load_pull_requests(
    tx: UnboundedSender<WorkerMessage>,
    client: octocrab::Octocrab,
    owner: Option<String>,
    repo: Option<String>,
) {
    tokio::spawn(async move {
        let message = match resolve_repository(owner, repo).await {
            Ok(repository) => {
                let label = repository.label();
                match fetch_open_pull_requests(&client, &repository).await {
                    Ok(pulls) => WorkerMessage::PullRequestsLoaded {
                        repository_label: label,
                        result: Ok(pulls),
                    },
                    Err(error) => WorkerMessage::PullRequestsLoaded {
                        repository_label: label,
                        result: Err(error.to_string()),
                    },
                }
            }
            Err(error) => WorkerMessage::PullRequestsLoaded {
                repository_label: "(unknown repository)".to_owned(),
                result: Err(error.to_string()),
            },
        };

        let _ = tx.send(message);
    });
}

/// Resolves and loads a specific pull request summary for direct-open startup.
pub fn spawn_load_specific_pull_request(
    tx: UnboundedSender<WorkerMessage>,
    client: octocrab::Octocrab,
    owner: Option<String>,
    repo: Option<String>,
    pull_number: u64,
) {
    tokio::spawn(async move {
        let message = match resolve_repository(owner, repo).await {
            Ok(repository) => {
                let label = repository.label();
                match fetch_pull_request_summary(&client, &repository, pull_number).await {
                    Ok(pull) => WorkerMessage::PullRequestResolved {
                        repository_label: label,
                        pull_number,
                        result: Ok(pull),
                    },
                    Err(error) => {
                        let detail = if error.is_not_found() {
                            format!(
                                "pull request #{pull_number} was not found in {}",
                                repository.label()
                            )
                        } else {
                            error.to_string()
                        };
                        WorkerMessage::PullRequestResolved {
                            repository_label: label,
                            pull_number,
                            result: Err(detail),
                        }
                    }
                }
            }
            Err(error) => WorkerMessage::PullRequestResolved {
                repository_label: "(unknown repository)".to_owned(),
                pull_number,
                result: Err(error.to_string()),
            },
        };

        let _ = tx.send(message);
    });
}

/// Spawns async loading of comments for a selected pull request.
pub fn spawn_load_pull_request_data(
    tx: UnboundedSender<WorkerMessage>,
    client: octocrab::Octocrab,
    pull: PullRequestSummary,
) {
    tokio::spawn(async move {
        let result = fetch_pull_request_data(&client, &pull)
            .await
            .map_err(|error| error.to_string());

        let _ = tx.send(WorkerMessage::PullRequestDataLoaded { pull, result });
    });
}

/// Spawns async loading of pull request diffs for the active pull request.
pub fn spawn_load_pull_request_diff(
    tx: UnboundedSender<WorkerMessage>,
    pull: PullRequestSummary,
    changed_files: Vec<String>,
) {
    tokio::spawn(async move {
        let result = fetch_pull_request_diff_data(&pull, &changed_files)
            .await
            .map_err(|error| error.to_string());
        let _ = tx.send(WorkerMessage::PullRequestDiffLoaded { pull, result });
    });
}

/// Spawns a mutation followed by a pull request comment refresh.
pub fn spawn_apply_mutation(
    tx: UnboundedSender<WorkerMessage>,
    client: octocrab::Octocrab,
    pull: PullRequestSummary,
    mutation: MutationRequest,
    clear_reply_root_key: Option<String>,
) {
    tokio::spawn(async move {
        let (mutation_result, clear_pending_review_comments) = match mutation {
            MutationRequest::ReplyToReviewComment {
                owner,
                repo,
                pull_number,
                comment_id,
                body,
            } => (
                reply_to_review_comment(&client, &owner, &repo, pull_number, comment_id, &body)
                    .await
                    .map(|_| ()),
                false,
            ),
            MutationRequest::SetReviewThreadResolved {
                thread_id,
                resolved,
            } => (
                set_review_thread_resolved(&client, &thread_id, resolved)
                    .await
                    .map(|_| ()),
                false,
            ),
            MutationRequest::SubmitPullRequestReview {
                owner,
                repo,
                pull_number,
                event,
                body,
                comments,
                expected_head_sha,
            } => (
                submit_pull_request_review(
                    &client,
                    SubmitPullRequestReviewRequest {
                        owner: &owner,
                        repo: &repo,
                        pull_number,
                        event: &event,
                        body: &body,
                        comments: &comments,
                        expected_head_sha: &expected_head_sha,
                    },
                )
                .await
                .map(|_| ()),
                true,
            ),
        };

        let result = match mutation_result {
            Ok(()) => fetch_pull_request_data(&client, &pull)
                .await
                .map_err(|error| error.to_string()),
            Err(error) => Err(error.to_string()),
        };

        let _ = tx.send(WorkerMessage::MutationApplied {
            pull,
            clear_reply_root_key,
            clear_pending_review_comments,
            result,
        });
    });
}
