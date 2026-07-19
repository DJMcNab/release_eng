//! Gets a first draft of a changelog

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize)]
/// Full JSON payload written by the tool.
pub struct Report {
    /// Repository in `owner/name` form.
    pub repository: String,
    /// Commit range used to generate this report.
    pub range: ReportRange,
    /// One entry per commit in the requested range.
    pub commits: Vec<CommitEntry>,
}

#[derive(Debug, Serialize)]
/// Commit range metadata for a report.
pub struct ReportRange {
    /// Start commit/range bound (exclusive).
    pub from: String,
    /// End commit/range bound (inclusive).
    pub to: String,
}

#[derive(Debug, Serialize)]
/// Pull-request data attached to a specific commit.
pub struct CommitEntry {
    /// Commit SHA.
    pub commit: String,
    /// Pull requests associated with this commit.
    pub prs: Vec<PullRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
/// Pull-request fields consumed from the GitHub API.
pub struct PullRequest {
    /// Pull request number.
    pub number: u64,
    /// Pull request title.
    pub title: String,
    /// Pull request body text.
    pub body: Option<String>,
    #[serde(rename = "html_url")]
    /// Pull request URL.
    pub url: String,
    /// Merge timestamp in RFC3339 format when available.
    pub merged_at: Option<String>,
    /// Pull request state from the GitHub API.
    pub state: String,
}

/// Builds a report for all commits in `from_commit..to_commit`.
pub fn fetch_pr_report(from_commit: &str, to_commit: &str) -> Result<Report> {
    verify_command_exists("git")?;
    verify_command_exists("gh")?;
    verify_commit_exists(from_commit, "from-commit")?;
    verify_commit_exists(to_commit, "to-commit")?;

    let repository = run_command(
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
    )
    .context("failed to resolve current GitHub repository via `gh repo view`")?;
    let repository = repository.trim().to_owned();
    if repository.is_empty() {
        bail!("`gh repo view` returned an empty repository name");
    }

    let range = format!("{from_commit}..{to_commit}");
    let rev_list_output = run_command("git", &["rev-list", "--reverse", &range])
        .context("failed to list commits in range")?;
    let commits: Vec<String> = rev_list_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if commits.is_empty() {
        bail!("no commits found in range {range}");
    }

    let mut commit_entries = Vec::with_capacity(commits.len());
    for commit in commits {
        let endpoint = format!("/repos/{repository}/commits/{commit}/pulls?per_page=100");
        let prs_json = run_command(
            "gh",
            &[
                "api",
                "-H",
                "Accept: application/vnd.github+json",
                &endpoint,
            ],
        )
        .with_context(|| format!("failed to fetch PRs associated with commit {commit}"))?;
        let prs: Vec<PullRequest> = serde_json::from_str(&prs_json)
            .with_context(|| format!("failed to parse PR JSON for commit {commit}"))?;
        commit_entries.push(CommitEntry { commit, prs });
    }

    Ok(Report {
        repository,
        range: ReportRange {
            from: from_commit.to_owned(),
            to: to_commit.to_owned(),
        },
        commits: commit_entries,
    })
}

/// Returns the default output path for a report.
pub fn default_output_path(from_commit: &str, to_commit: &str) -> PathBuf {
    PathBuf::from(format!(
        "pr_bodies_{}_{}.json",
        sanitize_for_filename(from_commit),
        sanitize_for_filename(to_commit)
    ))
}

/// Serializes a report to pretty JSON and writes it to `output_path`.
pub fn write_report_json(report: &Report, output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report).context("failed to serialize report JSON")?;
    fs::write(output_path, format!("{json}\n"))
        .with_context(|| format!("failed to write output file {}", output_path.display()))
}

fn verify_command_exists(name: &str) -> Result<()> {
    let status = Command::new(name)
        .arg("--version")
        .status()
        .with_context(|| format!("failed to execute `{name} --version`"))?;
    if !status.success() {
        bail!("required command `{name}` is not available");
    }
    Ok(())
}

fn verify_commit_exists(commit: &str, label: &str) -> Result<()> {
    let candidate = format!("{commit}^{{commit}}");
    run_command("git", &["rev-parse", "--verify", &candidate])
        .with_context(|| format!("invalid {label}: {commit}"))?;
    Ok(())
}

fn run_command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run command `{program}`"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let code = output.status.code().unwrap_or_default();
        if stderr.is_empty() {
            bail!("command `{program}` failed with exit code {code}");
        }
        bail!("command `{program}` failed with exit code {code}: {stderr}");
    }
    String::from_utf8(output.stdout).context("command output was not valid UTF-8")
}

fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::default_output_path;

    #[test]
    fn default_output_path_sanitizes_slashes() {
        let path = default_output_path("feature/foo", "main");
        assert_eq!(path.to_str(), Some("pr_bodies_feature_foo_main.json"));
    }
}
