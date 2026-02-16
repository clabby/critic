//! Pull request diff loading via local git clone + `difft --display json`.

use crate::{
    domain::{
        PullRequestDiffData, PullRequestDiffFile, PullRequestDiffFileStatus,
        PullRequestDiffHighlightRange, PullRequestDiffRow, PullRequestDiffRowKind,
        PullRequestSummary,
    },
    github::client::gh_auth_token,
};
use secrecy::ExposeSecret;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    env,
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{fs, process::Command};

/// Result type for pull request diff loading.
pub type Result<T> = std::result::Result<T, PullRequestDiffError>;

/// Errors returned while preparing repository state or parsing difft output.
#[derive(Debug, Error)]
pub enum PullRequestDiffError {
    #[error("HOME environment variable is not set")]
    MissingHomeDirectory,
    #[error("invalid path in changed files list: {0}")]
    InvalidChangedPath(String),
    #[error("failed to run git ({context}): {source}")]
    GitIo {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("git command failed ({context}) with status {status}: {stderr}")]
    GitFailed {
        context: &'static str,
        status: i32,
        stderr: String,
    },
    #[error("failed to run difft: {0}")]
    DifftIo(#[source] std::io::Error),
    #[error("difft failed with status {status}: {stderr}")]
    DifftFailed { status: i32, stderr: String },
    #[error("failed to parse difft JSON output: {0}")]
    DifftJson(#[from] serde_json::Error),
    #[error("failed to create diff workspace under {path}: {source}")]
    WorkspaceCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write snapshot file {path}: {source}")]
    SnapshotWrite {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Loads aligned per-file diff data for a pull request.
pub async fn fetch_pull_request_diff_data(
    pull: &PullRequestSummary,
    changed_files: &[String],
) -> Result<PullRequestDiffData> {
    if changed_files.is_empty() {
        return Ok(PullRequestDiffData { files: Vec::new() });
    }

    let git_auth = build_git_auth_header().await;
    let repo_dir = ensure_repo_available(pull, git_auth.as_deref()).await?;
    fetch_required_commits(
        &repo_dir,
        &pull.base_sha,
        &pull.head_sha,
        git_auth.as_deref(),
    )
    .await?;

    let (workspace_root, base_root, head_root) = create_workspace(pull).await?;
    let mut source_by_path = HashMap::<String, SourcePair>::new();

    for raw_path in changed_files {
        let normalized = normalize_changed_path(raw_path)?;

        let base_source =
            git_show_file(&repo_dir, &pull.base_sha, raw_path, git_auth.as_deref()).await?;
        let head_source =
            git_show_file(&repo_dir, &pull.head_sha, raw_path, git_auth.as_deref()).await?;
        if base_source.is_none() && head_source.is_none() {
            continue;
        }

        if let Some(base) = &base_source {
            write_snapshot_file(&base_root, &normalized, base).await?;
        }
        if let Some(head) = &head_source {
            write_snapshot_file(&head_root, &normalized, head).await?;
        }

        source_by_path.insert(
            normalized.clone(),
            SourcePair {
                base: base_source.unwrap_or_default(),
                head: head_source.unwrap_or_default(),
            },
        );
    }

    let parsed = run_difft_json(&base_root, &head_root).await;
    let _ = fs::remove_dir_all(&workspace_root).await;
    let difft_files = parsed?;

    let mut parsed_by_path = HashMap::<String, RawDifftFile>::with_capacity(difft_files.len());
    for file in difft_files {
        parsed_by_path.insert(normalize_path_for_lookup(&file.path), file);
    }

    let mut files = Vec::new();
    for raw_path in changed_files {
        let normalized = normalize_changed_path(raw_path)?;
        let Some(source) = source_by_path.get(&normalized) else {
            continue;
        };
        let parsed = parsed_by_path.remove(&normalized);
        files.push(build_diff_file(&normalized, source, parsed));
    }

    Ok(PullRequestDiffData { files })
}

#[derive(Debug, Clone)]
struct SourcePair {
    base: String,
    head: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawDifftOutput {
    File(RawDifftFile),
    Files(Vec<RawDifftFile>),
}

#[derive(Debug, Clone, Deserialize)]
struct RawDifftFile {
    path: String,
    status: String,
    #[serde(default)]
    aligned_lines: Vec<[Option<usize>; 2]>,
    #[serde(default)]
    chunks: Vec<Vec<RawDifftChunkLine>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDifftChunkLine {
    lhs: Option<RawDifftSide>,
    rhs: Option<RawDifftSide>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDifftSide {
    line_number: usize,
    #[serde(default)]
    changes: Vec<RawDifftChange>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDifftChange {
    start: usize,
    end: usize,
}

async fn ensure_repo_available(
    pull: &PullRequestSummary,
    git_auth: Option<&str>,
) -> Result<PathBuf> {
    let repo_dir = repo_cache_dir(pull)?;
    if repo_dir.exists() {
        return Ok(repo_dir);
    }

    if let Some(parent) = repo_dir.parent() {
        fs::create_dir_all(parent).await.map_err(|source| {
            PullRequestDiffError::WorkspaceCreate {
                path: parent.display().to_string(),
                source,
            }
        })?;
    }

    let remote = format!("https://github.com/{}/{}.git", pull.owner, pull.repo);
    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            remote.as_str(),
            repo_dir.to_string_lossy().as_ref(),
        ],
        None,
        "clone repository",
        git_auth,
    )
    .await?;

    Ok(repo_dir)
}

async fn fetch_required_commits(
    repo_dir: &Path,
    base_sha: &str,
    head_sha: &str,
    git_auth: Option<&str>,
) -> Result<()> {
    let mut missing = Vec::<String>::new();
    if !commit_exists(repo_dir, base_sha, git_auth).await? {
        missing.push(base_sha.to_owned());
    }
    if base_sha != head_sha && !commit_exists(repo_dir, head_sha, git_auth).await? {
        missing.push(head_sha.to_owned());
    }
    if missing.is_empty() {
        return Ok(());
    }

    let mut unique = HashSet::<String>::with_capacity(missing.len());
    let refs = missing
        .into_iter()
        .filter(|sha| unique.insert(sha.clone()))
        .collect::<Vec<_>>();
    fetch_commits(repo_dir, &refs, git_auth).await?;
    Ok(())
}

async fn commit_exists(repo_dir: &Path, sha: &str, git_auth: Option<&str>) -> Result<bool> {
    let mut command = git_command(git_auth);
    command
        .arg("-C")
        .arg(repo_dir)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{sha}^{{commit}}"));
    let output = command
        .output()
        .await
        .map_err(|source| PullRequestDiffError::GitIo {
            context: "check commit presence",
            source,
        })?;

    Ok(output.status.success())
}

async fn fetch_commits(repo_dir: &Path, refs: &[String], git_auth: Option<&str>) -> Result<()> {
    let mut command = git_command(git_auth);
    command.arg("-C").arg(repo_dir).arg("fetch").arg("origin");
    for reference in refs {
        command.arg(reference);
    }

    let output = command
        .output()
        .await
        .map_err(|source| PullRequestDiffError::GitIo {
            context: "fetch required commits",
            source,
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(PullRequestDiffError::GitFailed {
        context: "fetch required commits",
        status: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

async fn create_workspace(pull: &PullRequestSummary) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let root = runtime_root()?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let workspace = root.join("work").join(format!(
        "pr-diff-{}-{}-{}",
        pull.number,
        std::process::id(),
        millis
    ));
    let base = workspace.join("base");
    let head = workspace.join("head");

    fs::create_dir_all(&base)
        .await
        .map_err(|source| PullRequestDiffError::WorkspaceCreate {
            path: base.display().to_string(),
            source,
        })?;
    fs::create_dir_all(&head)
        .await
        .map_err(|source| PullRequestDiffError::WorkspaceCreate {
            path: head.display().to_string(),
            source,
        })?;

    Ok((workspace, base, head))
}

async fn git_show_file(
    repo_dir: &Path,
    sha: &str,
    path: &str,
    git_auth: Option<&str>,
) -> Result<Option<String>> {
    let spec = format!("{sha}:{path}");
    let mut command = git_command(git_auth);
    command.arg("-C").arg(repo_dir).arg("show").arg(spec);
    let output = command
        .output()
        .await
        .map_err(|source| PullRequestDiffError::GitIo {
            context: "show file content",
            source,
        })?;

    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

async fn write_snapshot_file(root: &Path, relative_path: &str, content: &str) -> Result<()> {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|source| {
            PullRequestDiffError::WorkspaceCreate {
                path: parent.display().to_string(),
                source,
            }
        })?;
    }
    fs::write(&path, content)
        .await
        .map_err(|source| PullRequestDiffError::SnapshotWrite {
            path: path.display().to_string(),
            source,
        })?;
    Ok(())
}

async fn run_difft_json(base_root: &Path, head_root: &Path) -> Result<Vec<RawDifftFile>> {
    let output = Command::new("difft")
        .env("DFT_UNSTABLE", "yes")
        .arg("--display")
        .arg("json")
        .arg("--color")
        .arg("never")
        .arg(base_root)
        .arg(head_root)
        .output()
        .await
        .map_err(PullRequestDiffError::DifftIo)?;

    if !output.status.success() {
        return Err(PullRequestDiffError::DifftFailed {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let parsed: RawDifftOutput = serde_json::from_slice(&output.stdout)?;
    Ok(match parsed {
        RawDifftOutput::File(file) => vec![file],
        RawDifftOutput::Files(files) => files,
    })
}

fn build_diff_file(
    path: &str,
    source: &SourcePair,
    parsed: Option<RawDifftFile>,
) -> PullRequestDiffFile {
    let left_lines = split_preserving_trailing_newline(&source.base);
    let right_lines = split_preserving_trailing_newline(&source.head);

    let status = parsed
        .as_ref()
        .map(|entry| status_from_difft(&entry.status))
        .unwrap_or_else(|| fallback_status(&left_lines, &right_lines));

    let aligned = parsed
        .as_ref()
        .map(|entry| entry.aligned_lines.clone())
        .filter(|lines| !lines.is_empty())
        .unwrap_or_else(|| fallback_aligned_lines(&left_lines, &right_lines, status));

    let (changed_left, changed_right, left_highlights, right_highlights) = parsed
        .as_ref()
        .map(changed_line_annotations)
        .unwrap_or_default();

    let mut rows = Vec::with_capacity(aligned.len());
    for pair in &aligned {
        let left_index = pair[0];
        let right_index = pair[1];
        let left_text = left_index
            .and_then(|index| left_lines.get(index))
            .cloned()
            .unwrap_or_default();
        let right_text = right_index
            .and_then(|index| right_lines.get(index))
            .cloned()
            .unwrap_or_default();

        let kind = classify_row_kind(left_index, right_index, &changed_left, &changed_right);
        rows.push(PullRequestDiffRow {
            left_line_number: left_index.map(|value| value + 1),
            right_line_number: right_index.map(|value| value + 1),
            left_text,
            right_text,
            left_highlights: left_index
                .and_then(|index| left_highlights.get(&index).cloned())
                .unwrap_or_default(),
            right_highlights: right_index
                .and_then(|index| right_highlights.get(&index).cloned())
                .unwrap_or_default(),
            kind,
        });
    }

    trim_terminal_empty_row(&mut rows);

    let mut hunk_starts = parsed
        .as_ref()
        .map(|entry| hunk_row_starts(&rows, &aligned, &entry.chunks))
        .unwrap_or_default();
    if hunk_starts.is_empty() && !rows.is_empty() {
        hunk_starts.push(0);
    }

    PullRequestDiffFile {
        path: path.to_owned(),
        status,
        rows,
        hunk_starts,
    }
}

fn split_preserving_trailing_newline(value: &str) -> Vec<String> {
    value.split('\n').map(|line| line.to_owned()).collect()
}

fn status_from_difft(value: &str) -> PullRequestDiffFileStatus {
    match value {
        "created" => PullRequestDiffFileStatus::Added,
        "deleted" => PullRequestDiffFileStatus::Removed,
        _ => PullRequestDiffFileStatus::Modified,
    }
}

fn fallback_status(left_lines: &[String], right_lines: &[String]) -> PullRequestDiffFileStatus {
    if left_lines.iter().all(|line| line.is_empty()) {
        PullRequestDiffFileStatus::Added
    } else if right_lines.iter().all(|line| line.is_empty()) {
        PullRequestDiffFileStatus::Removed
    } else {
        PullRequestDiffFileStatus::Modified
    }
}

fn fallback_aligned_lines(
    left_lines: &[String],
    right_lines: &[String],
    status: PullRequestDiffFileStatus,
) -> Vec<[Option<usize>; 2]> {
    match status {
        PullRequestDiffFileStatus::Added => (0..right_lines.len())
            .map(|index| [None, Some(index)])
            .collect(),
        PullRequestDiffFileStatus::Removed => (0..left_lines.len())
            .map(|index| [Some(index), None])
            .collect(),
        PullRequestDiffFileStatus::Modified => {
            let max = left_lines.len().max(right_lines.len());
            (0..max)
                .map(|index| {
                    let left = (index < left_lines.len()).then_some(index);
                    let right = (index < right_lines.len()).then_some(index);
                    [left, right]
                })
                .collect()
        }
    }
}

type HighlightMap = HashMap<usize, Vec<PullRequestDiffHighlightRange>>;

fn changed_line_annotations(
    entry: &RawDifftFile,
) -> (
    HashMap<usize, ()>,
    HashMap<usize, ()>,
    HighlightMap,
    HighlightMap,
) {
    let mut left = HashMap::new();
    let mut right = HashMap::new();
    let mut left_highlights = HashMap::<usize, Vec<PullRequestDiffHighlightRange>>::new();
    let mut right_highlights = HashMap::<usize, Vec<PullRequestDiffHighlightRange>>::new();

    for chunk in &entry.chunks {
        for line in chunk {
            if let Some(lhs) = &line.lhs {
                left.insert(lhs.line_number, ());
                let ranges = lhs
                    .changes
                    .iter()
                    .filter_map(normalize_change_range)
                    .collect::<Vec<_>>();
                if !ranges.is_empty() {
                    left_highlights
                        .entry(lhs.line_number)
                        .or_default()
                        .extend(ranges);
                }
            }
            if let Some(rhs) = &line.rhs {
                right.insert(rhs.line_number, ());
                let ranges = rhs
                    .changes
                    .iter()
                    .filter_map(normalize_change_range)
                    .collect::<Vec<_>>();
                if !ranges.is_empty() {
                    right_highlights
                        .entry(rhs.line_number)
                        .or_default()
                        .extend(ranges);
                }
            }
        }
    }

    normalize_highlight_map(&mut left_highlights);
    normalize_highlight_map(&mut right_highlights);

    (left, right, left_highlights, right_highlights)
}

fn normalize_change_range(change: &RawDifftChange) -> Option<PullRequestDiffHighlightRange> {
    (change.start < change.end).then_some(PullRequestDiffHighlightRange {
        start: change.start,
        end: change.end,
    })
}

fn normalize_highlight_map(map: &mut HighlightMap) {
    for ranges in map.values_mut() {
        *ranges = merge_highlight_ranges(ranges);
    }
}

fn merge_highlight_ranges(
    ranges: &[PullRequestDiffHighlightRange],
) -> Vec<PullRequestDiffHighlightRange> {
    if ranges.is_empty() {
        return Vec::new();
    }

    let mut sorted = ranges.to_vec();
    sorted.sort_unstable_by_key(|range| (range.start, range.end));

    let mut merged: Vec<PullRequestDiffHighlightRange> = Vec::with_capacity(sorted.len());
    for range in sorted {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => {
                last.end = last.end.max(range.end);
            }
            _ => merged.push(range),
        }
    }

    merged
}

fn classify_row_kind(
    left_index: Option<usize>,
    right_index: Option<usize>,
    changed_left: &HashMap<usize, ()>,
    changed_right: &HashMap<usize, ()>,
) -> PullRequestDiffRowKind {
    match (left_index, right_index) {
        (None, Some(_)) => PullRequestDiffRowKind::Added,
        (Some(_), None) => PullRequestDiffRowKind::Removed,
        (Some(left), Some(right))
            if changed_left.contains_key(&left) || changed_right.contains_key(&right) =>
        {
            PullRequestDiffRowKind::Modified
        }
        _ => PullRequestDiffRowKind::Context,
    }
}

fn hunk_row_starts(
    rows: &[PullRequestDiffRow],
    aligned: &[[Option<usize>; 2]],
    chunks: &[Vec<RawDifftChunkLine>],
) -> Vec<usize> {
    let mut starts = Vec::new();

    for chunk in chunks {
        let mut left_lines = HashSet::new();
        let mut right_lines = HashSet::new();
        for line in chunk {
            if let Some(lhs) = &line.lhs {
                left_lines.insert(lhs.line_number);
            }
            if let Some(rhs) = &line.rhs {
                right_lines.insert(rhs.line_number);
            }
        }

        let row_index = aligned.iter().enumerate().find_map(|(index, pair)| {
            let left_hit = pair[0].is_some_and(|value| left_lines.contains(&value));
            let right_hit = pair[1].is_some_and(|value| right_lines.contains(&value));
            (left_hit || right_hit).then_some(index)
        });

        if let Some(index) = row_index
            && starts.last().copied() != Some(index)
            && index < rows.len()
        {
            starts.push(index);
        }
    }

    starts
}

fn trim_terminal_empty_row(rows: &mut Vec<PullRequestDiffRow>) {
    if rows.last().is_some_and(|row| {
        row.left_text.is_empty()
            && row.right_text.is_empty()
            && row.left_line_number.is_some()
            && row.right_line_number.is_some()
    }) {
        rows.pop();
    }
}

fn normalize_changed_path(path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(PullRequestDiffError::InvalidChangedPath(path.to_owned()));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(PullRequestDiffError::InvalidChangedPath(path.to_owned()));
    }

    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(segment) => parts.push(segment.to_string_lossy().to_string()),
            _ => return Err(PullRequestDiffError::InvalidChangedPath(path.to_owned())),
        }
    }

    if parts.is_empty() {
        return Err(PullRequestDiffError::InvalidChangedPath(path.to_owned()));
    }

    Ok(parts.join("/"))
}

fn normalize_path_for_lookup(path: &str) -> String {
    path.replace('\\', "/")
}

fn runtime_root() -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or(PullRequestDiffError::MissingHomeDirectory)?;
    Ok(PathBuf::from(home).join(".critic"))
}

fn repo_cache_dir(pull: &PullRequestSummary) -> Result<PathBuf> {
    Ok(runtime_root()?
        .join("repos")
        .join(&pull.owner)
        .join(&pull.repo))
}

async fn run_git(
    args: &[&str],
    cwd: Option<&Path>,
    context: &'static str,
    git_auth: Option<&str>,
) -> Result<()> {
    let mut command = git_command(git_auth);
    command.args(args);
    if let Some(path) = cwd {
        command.current_dir(path);
    }

    let output = command
        .output()
        .await
        .map_err(|source| PullRequestDiffError::GitIo { context, source })?;

    if output.status.success() {
        return Ok(());
    }

    Err(PullRequestDiffError::GitFailed {
        context,
        status: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

fn git_command(git_auth: Option<&str>) -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null());

    if let Some(extra_header) = git_auth {
        command.arg("-c").arg(format!(
            "http.https://github.com/.extraheader={extra_header}"
        ));
    }

    command
}

async fn build_git_auth_header() -> Option<String> {
    let token = gh_auth_token().await.ok()?;
    let credential = format!("x-access-token:{}", token.expose_secret());
    let encoded = base64_encode(credential.as_bytes());
    Some(format!("AUTHORIZATION: basic {encoded}"))
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut index = 0usize;

    while index < input.len() {
        let a = input[index];
        let b = input.get(index + 1).copied();
        let c = input.get(index + 2).copied();

        let n = (u32::from(a) << 16) | (u32::from(b.unwrap_or(0)) << 8) | u32::from(c.unwrap_or(0));
        output.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        output.push(b.map_or('=', |_| TABLE[((n >> 6) & 0x3f) as usize] as char));
        output.push(c.map_or('=', |_| TABLE[(n & 0x3f) as usize] as char));

        index += 3;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_changed_path_rejects_parent_traversal() {
        let result = normalize_changed_path("../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn build_diff_file_populates_hunk_starts() {
        let source = SourcePair {
            base: "a\nb\n".to_owned(),
            head: "a\nc\n".to_owned(),
        };
        let parsed = RawDifftFile {
            path: "example.rs".to_owned(),
            status: "changed".to_owned(),
            aligned_lines: vec![[Some(0), Some(0)], [Some(1), Some(1)], [Some(2), Some(2)]],
            chunks: vec![vec![RawDifftChunkLine {
                lhs: Some(RawDifftSide {
                    line_number: 1,
                    changes: vec![RawDifftChange { start: 0, end: 1 }],
                }),
                rhs: Some(RawDifftSide {
                    line_number: 1,
                    changes: vec![RawDifftChange { start: 0, end: 1 }],
                }),
            }]],
        };

        let file = build_diff_file("example.rs", &source, Some(parsed));

        assert_eq!(file.hunk_starts, vec![1]);
        assert_eq!(file.rows.len(), 2);
        assert_eq!(file.rows[1].kind, PullRequestDiffRowKind::Modified);
    }

    #[test]
    fn build_diff_file_attaches_merged_highlight_ranges() {
        let source = SourcePair {
            base: "let value = foo();\n".to_owned(),
            head: "let value = bar();\n".to_owned(),
        };
        let parsed = RawDifftFile {
            path: "example.rs".to_owned(),
            status: "changed".to_owned(),
            aligned_lines: vec![[Some(0), Some(0)], [Some(1), Some(1)]],
            chunks: vec![vec![RawDifftChunkLine {
                lhs: Some(RawDifftSide {
                    line_number: 0,
                    changes: vec![
                        RawDifftChange { start: 12, end: 14 },
                        RawDifftChange { start: 14, end: 15 },
                    ],
                }),
                rhs: Some(RawDifftSide {
                    line_number: 0,
                    changes: vec![RawDifftChange { start: 12, end: 15 }],
                }),
            }]],
        };

        let file = build_diff_file("example.rs", &source, Some(parsed));
        let row = &file.rows[0];

        assert_eq!(
            row.left_highlights,
            vec![PullRequestDiffHighlightRange { start: 12, end: 15 }]
        );
        assert_eq!(
            row.right_highlights,
            vec![PullRequestDiffHighlightRange { start: 12, end: 15 }]
        );
    }
}
