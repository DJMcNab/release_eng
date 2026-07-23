// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! First-draft changelog generation for single-or-multi crate projects, for use as an `xtask`.
//!
//! This project currently requires that the repository is hosted on GitHub, and uses squash merges.
//! This is the process used by Linebender.
//! It supports routing changelog entries to all impacted crates for crates in a workspace which
//! maintain their own changelogs.
//!
//! # Usage
//!
//! In a repository which has this setup as an xtask, run it as:
//!
//! ```sh
//! cargo xtask generate-changelog
//! ```
//!
//! After ensuring that you're on the primary branch of the repository.
//! Once this command finishes, each CHANGELOG in the repository will have unstaged changes.
//! Use these as a starting point to create the new release's changelog.
//!
//! The entries are collected from a quoted section in each PR which follows a **Changelog** marker.
//! To explicitly indicate that a PR does not require a changelog entry, replace this with **Changelog: None**.
//!
//! The entries will be merged into the 'Unreleased' section of the relevant CHANGELOG, inferred from the
//! files changed in the PR.
//! You must then manually review these entries, and edit the CHANGELOG based on them.
//!
//! # Motivation
//!
//! A traditional workflow for as-you-go changelog generation is for all PRs with relevant
//! changes to also edit the CHANGELOG.md file to add their entry.
//! However, as we've used this in Linebender, we ran into several issues:
//!
//! - It isn't clear if a changelog has been forgotten, or if the author intentionally decided it wasn't needed.
//! - It's very easy for edits in the CHANGELOG file to generate conflicts.
//! - It's possible for CHANGELOG entries to accidentally end up in the wrong place, if a
//!   release happens between a PR being opened and merged.
//!
//! Systems which track in-progress changelogs using in-tree files avoid conflicts, but have issues
//! with approachability for users.
//! They also require choosing where to store the data.
//! This approach avoids this by storing the data in a PR description, which a contributor will already need to fill out.
//! Additionally, this gives maintainers a low-friction way to edit the changelog entry for a PR, either before or after merge.
//!
//! # Setup
//!
//! See the docs on [`Config`] for detailed setup instructions.
//!
//! # Inspirations
//!
//! The workflow in this crate is inspired by the conventions used in the Clippy repository (<https://github.com/rust-lang/rust-clippy>).
//! We however automate the process slightly more than is done in Clippy, to make updating changelogs require less manual work.

mod apply;
mod coauthors;
mod collect;
mod extract;
mod merge;
mod process;

pub(crate) use apply::apply;
pub(crate) use collect::collect;

use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Command-line arguments accepted by a `git-changelog`-based binary.
#[derive(clap::Parser, Debug, Clone, Default)]
pub struct Args {
    /// Git ref to start the search from.
    ///
    /// Only required on the very first run, when the default changelog has no
    /// `git-changelog:last-commit` marker yet.
    /// It is invalid to provide this on later runs.
    #[arg(long)]
    pub since: Option<String>,
}

/// Builder for a changelog generation CLI.
///
/// # Requirements
///
/// This requires that your repository is hosted on GitHub.
/// We currently only support repositories hosted on `github.com`.
/// The repository may have more than one CHANGELOG file, which are specified
/// using [`changelog`](Self::changelog).
///
/// This tool requires that there workspace has a top-level CHANGELOG file.
/// This is used for all changes which request a changelog entry but which can't
/// be routed to a specific crate.
/// It also assumes that both `git` and the GitHub CLI (`gh`) are installed and available
/// on `PATH`.
///
/// Some files (by default, `Cargo.toml` and `Cargo.lock`) are ignored for routing.
/// See [`ignore_paths`](Self::ignore_paths) for details.
///
/// # Setup
///
/// You should set this up as an xtask in your repository.
/// See <https://github.com/matklad/cargo-xtask> for documentation of the pattern.
/// A working setup can also be found in <https://github.com/DJMcNab/release_eng>.
///
/// The basic steps are to `cargo new xtask`; `cargo add -p xtask generate-changelog clap anyhow`
/// Then filling the new `xtask/src/main.rs` to call this library with appropriate context, e.g.:
///
///  ```no_run
/// use anyhow::Result;
/// use clap::{Parser, Subcommand};
/// use git_changelog::{Args, Config};
///
/// #[derive(Parser, Debug)]
/// #[command(bin_name = "cargo xtask")]
/// struct Cli {
///     #[command(subcommand)]
///     command: Command,
/// }
///
/// #[derive(Subcommand, Debug)]
/// enum Command {
///     /// Generate a first-draft changelog from merged pull requests.
///     GenerateChangelog(Args),
/// }
///
/// fn main() -> Result<()> {
///     let cli = Cli::parse();
///     match cli.command {
///         Command::GenerateChangelog(args) => Config::new("linebender/vello", "CHANGELOG.md")
///             .changelog(
///                 "sparse_strips/vello_cpu/CHANGELOG.md",
///                 ["sparse_strips/vello_cpu"],
///             )
///             .run(args),
///     }
/// }
/// ```
///
/// You would then also want to add the appropriate entry to your `.cargo/config.toml`.
#[derive(Debug, Clone)]
#[must_use = "Does nothing until `run` is called."]
pub struct Config {
    /// The repository in `owner/name` form.
    pub(crate) repo: String,
    /// The primary changelog file.
    pub(crate) default_changelog: PathBuf,
    pub(crate) changelogs: Vec<ChangelogTarget>,
    /// Repo-root-relative paths ignored for routing purposes.
    pub(crate) ignore_paths: Vec<PathBuf>,
}

impl Config {
    /// Creates a new configuration.
    ///
    /// `repo` is the GitHub repository which pull requests are resolved against, in `owner/name` form.
    /// `default_changelog` is the repo-root-relative path to the primary changelog file.
    /// This changelog is also used to store which commit the changelog most recently covers.
    pub fn new(repo: impl Into<String>, default_changelog: impl Into<PathBuf>) -> Self {
        Self {
            repo: repo.into(),
            default_changelog: default_changelog.into(),
            changelogs: Vec::new(),
            ignore_paths: default_ignore_paths(),
        }
    }

    /// Registers an additional changelog file.
    ///
    /// Changes in any of the source roots in `roots` are routed to this changelog file.
    /// All paths are relative to the repository root.
    pub fn changelog<R, P>(mut self, file: impl Into<PathBuf>, roots: R) -> Self
    where
        R: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.changelogs.push(ChangelogTarget {
            file: file.into(),
            roots: roots.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Sets the files which are ignored for routing purposes.
    ///
    /// Defaults to `["Cargo.toml", "Cargo.lock"]`.
    /// This means that if you add a dependency in a PR to a sub crate, the root changelog
    /// won't have it inserted.
    pub fn ignore_paths<R, P>(mut self, paths: R) -> Self
    where
        R: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.ignore_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Updates all contained changelogs to include all PRs since the last run.
    pub fn run(self, args: Args) -> Result<()> {
        let collected = collect(&self, &args)?;
        apply(&self, &collected)
    }

    /// All changelog files known to this configuration (the default plus every registered
    /// extra changelog), deduplicated.
    pub(crate) fn all_changelog_files(&self) -> BTreeSet<PathBuf> {
        let mut files: BTreeSet<PathBuf> = self.changelogs.iter().map(|t| t.file.clone()).collect();
        files.insert(self.default_changelog.clone());
        files
    }
}

fn default_ignore_paths() -> Vec<PathBuf> {
    vec![PathBuf::from("Cargo.toml"), PathBuf::from("Cargo.lock")]
}

/// One or more source roots that route to a single changelog file.
#[derive(Debug, Clone)]
pub(crate) struct ChangelogTarget {
    /// The changelog file this target routes to, repo-root-relative.
    pub(crate) file: PathBuf,
    /// Source roots that route a PR's (non-ignored) changed paths to `file`.
    pub(crate) roots: Vec<PathBuf>,
}

/// Data gathered about a single merged pull request.
#[derive(Debug, Clone)]
pub(crate) struct PrData {
    /// The pull request number.
    pub(crate) number: u64,
    /// The pull request title.
    pub(crate) title: String,
    /// The pull request body (description), if any.
    pub(crate) body: Option<String>,
    /// The GitHub login of the PR author.
    pub(crate) author: String,
    /// GitHub logins parsed from `Co-authored-by:` trailers on the squash-merge commit.
    pub(crate) co_authors: Vec<String>,
    /// Repo-root-relative paths changed by this PR's squash-merge commit.
    pub(crate) changed_paths: Vec<PathBuf>,
}

/// Everything gathered from `git` and `gh` for a single run. See `collect`.
#[derive(Debug, Clone)]
pub(crate) struct Collected {
    /// The commit SHA at `HEAD`, to which the marker will be advanced.
    pub(crate) head_sha: String,
    /// Data for every merged PR found in the collected range, in merge order.
    pub(crate) prs: Vec<PrData>,
}

/// Per-run statistics, printed to stderr by `apply`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Summary {
    /// Number of entries added, per changelog file.
    pub(crate) entries_added: BTreeMap<PathBuf, usize>,
    /// Number of `no-changelog: <title>` placeholder entries added across all files.
    pub(crate) placeholders: usize,
    /// Number of PRs skipped entirely because they opted out with `Changelog: None`.
    pub(crate) none_skipped: usize,
}

/// The result of the pure processing stage. See `process`.
#[derive(Debug, Clone)]
pub(crate) struct ProcessOutput {
    /// New file contents, keyed by repo-root-relative path. Only files that actually changed
    /// are present.
    pub(crate) updated: BTreeMap<PathBuf, String>,
    /// Run statistics for the end-of-run summary.
    pub(crate) summary: Summary,
}
