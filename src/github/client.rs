//! Authenticated Octocrab client bootstrap.

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;
use tokio::process::Command;

/// Result type for GitHub client bootstrap.
pub type Result<T> = std::result::Result<T, GitHubClientError>;

/// Errors returned while loading a token and creating an Octocrab client.
#[derive(Debug, Error)]
pub enum GitHubClientError {
    #[error("failed to run `gh auth token`; ensure GitHub CLI is installed ({0})")]
    GhNotAvailable(std::io::Error),
    #[error("`gh auth token` failed with status {status}: {stderr}")]
    GhAuthFailed { status: i32, stderr: String },
    #[error("`gh auth token` returned an empty token")]
    InvalidToken,
    #[error("failed to initialize octocrab client: {0}")]
    Octocrab(octocrab::Error),
}

/// Builds an authenticated Octocrab client from `gh auth token`.
pub async fn create_client() -> Result<octocrab::Octocrab> {
    let token = gh_auth_token().await?;

    octocrab::OctocrabBuilder::new()
        .personal_token(token.expose_secret().to_owned())
        .build()
        .map_err(GitHubClientError::Octocrab)
}

/// Returns the active GitHub token from the `gh` CLI.
pub async fn gh_auth_token() -> Result<SecretString> {
    let output = Command::new("gh")
        .arg("auth")
        .arg("token")
        .output()
        .await
        .map_err(GitHubClientError::GhNotAvailable)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(GitHubClientError::GhAuthFailed {
            status: output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if token.is_empty() {
        return Err(GitHubClientError::InvalidToken);
    }

    Ok(SecretString::from(token))
}
