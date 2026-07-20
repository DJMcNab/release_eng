//! Stage 1: collect (impure).
//!
//! Talks to `git` and `gh` to gather everything [`crate::process`] needs, with no further I/O.

use crate::apply::read_marker;
use crate::coauthors::resolve_pr_co_authors;
use crate::{Args, Collected, Config, PrData};
use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Gathers everything needed from `git` and `gh` so that [`crate::process`] needs no I/O of its
/// own.
///
/// Reads the `git-changelog:last-commit` marker from `config`'s default changelog (falling back
/// to `args.since` if absent -- and erroring if both are absent), lists the commits since that
/// point, extracts merged PR numbers from `(#N)`-suffixed commit subjects, and fetches each PR's
/// data via `gh api` plus local `git` metadata.
pub(crate) fn collect(config: &Config, args: &Args) -> Result<Collected> {
    verify_command_exists("git")?;
    verify_command_exists("gh")?;

    let default_content = fs::read_to_string(&config.default_changelog).with_context(|| {
        format!(
            "failed to read default changelog {}",
            config.default_changelog.display()
        )
    })?;
    let marker = read_marker(&default_content);

    let base = match (marker, &args.since) {
        (Some(sha), _) => sha,
        (None, Some(since)) => since.clone(),
        (None, None) => bail!(
            "no `git-changelog:last-commit` marker found in {}; pass --since <git-ref> for the first run",
            config.default_changelog.display()
        ),
    };

    let head_sha = run_git(&["rev-parse", "HEAD"])?.trim().to_owned();
    let range = format!("{base}..HEAD");
    let commits: Vec<String> = run_git(&["rev-list", "--reverse", &range])
        .with_context(|| format!("failed to list commits in range {range}"))?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let mut seen = BTreeSet::new();
    let mut ordered_prs: Vec<(u64, String)> = Vec::new();
    for sha in commits {
        let subject = run_git(&["log", "-1", "--format=%s", &sha])
            .with_context(|| format!("failed to read subject of commit {sha}"))?;
        let Some(number) = parse_trailing_pr_number(subject.trim()) else {
            continue;
        };
        if seen.insert(number) {
            ordered_prs.push((number, sha));
        }
    }

    let mut prs = Vec::with_capacity(ordered_prs.len());
    for (number, sha) in ordered_prs {
        let endpoint = format!("/repos/{}/pulls/{number}", config.repo);
        let api_json = run_gh(&["api", "-H", "Accept: application/vnd.github+json", &endpoint])
            .with_context(|| format!("failed to fetch PR #{number} via `gh api`"))?;
        let api: PrApiResponse = serde_json::from_str(&api_json)
            .with_context(|| format!("failed to parse PR JSON for #{number}"))?;

        let commit_message = run_git(&["log", "-1", "--format=%B", &sha])
            .with_context(|| format!("failed to read commit message for {sha}"))?;
        let co_authors = resolve_pr_co_authors(&config.repo, number, &commit_message);

        let changed_paths: Vec<PathBuf> = run_git(&["show", "--name-only", "--pretty=format:", &sha])
            .with_context(|| format!("failed to read changed paths for {sha}"))?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        prs.push(PrData {
            number,
            title: api.title,
            body: api.body,
            author: api.user.login,
            co_authors,
            changed_paths,
        });
    }

    Ok(Collected { head_sha, prs })
}

#[derive(Debug, Deserialize)]
struct PrApiResponse {
    title: String,
    body: Option<String>,
    user: PrApiUser,
}

#[derive(Debug, Deserialize)]
struct PrApiUser {
    login: String,
}

/// Parses the trailing `(#N)` PR-number suffix from a squash-merge commit subject.
fn parse_trailing_pr_number(subject: &str) -> Option<u64> {
    let subject = subject.trim_end();
    let subject = subject.strip_suffix(')')?;
    let idx = subject.rfind("(#")?;
    let digits = &subject[idx + 2..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
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

/// Runs `git` with the given arguments and returns its stdout.
///
/// Shared with [`crate::apply`]'s pre-flight dirty check.
pub(crate) fn run_git(args: &[&str]) -> Result<String> {
    run_command("git", args)
}

/// Runs `gh` with the given arguments and returns its stdout.
///
/// Shared with [`crate::coauthors`]'s PR-commit lookup.
pub(crate) fn run_gh(args: &[&str]) -> Result<String> {
    run_command("gh", args)
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
