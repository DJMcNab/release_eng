//! A reusable first-draft changelog generator.
//!
//! Linebender projects keep hand-curated, keep-a-changelog-style `CHANGELOG.md` files: a
//! `## [Unreleased]` section, `### Added`/`### Changed`/... subsections, entries suffixed
//! `([#1234][] by [@author][])`, and reference-style link definitions collected at the bottom
//! of the file.
//!
//! This crate finds the pull requests merged since the last run, extracts their changelog
//! sections from their PR bodies, routes each PR to the right changelog file(s) based on which
//! paths it touched, and merges the resulting entries into `## [Unreleased]`, sorted by PR
//! number. The output is a first draft: a human is expected to curate it afterwards.
//!
//! The pipeline has three stages:
//!
//! 1. `collect` (impure: talks to `git` and `gh`) gathers everything needed from the outside
//!    world into a `Collected` value.
//! 2. `process` (pure: no I/O at all) takes a `Collected` plus the current contents of the
//!    target changelog files and computes the new file contents.
//! 3. `apply` (impure) does a pre-flight dirty check, reads the current file contents, calls
//!    `process`, writes the results, and advances the marker last.
//!
//! A calling `xtask` looks like:
//!
//! ```no_run
//! # fn main() -> anyhow::Result<()> {
//! use clap::Parser as _;
//! git_changelog::Config::new("linebender/vello", "CHANGELOG.md")
//!     .changelog("sparse_strips/vello_cpu/CHANGELOG.md", ["sparse_strips/vello_cpu"])
//!     .changelog("sparse_strips/vello_common/CHANGELOG.md", ["sparse_strips/vello_common"])
//!     .run(git_changelog::Args::parse())?;
//! # Ok(())
//! # }
//! ```

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
    /// `git-changelog:last-commit` marker yet. Later runs read the marker instead.
    #[arg(long)]
    pub since: Option<String>,
}

/// One or more source roots that route to a single changelog file.
#[derive(Debug, Clone)]
pub(crate) struct ChangelogTarget {
    /// The changelog file this target routes to, repo-root-relative.
    pub(crate) file: PathBuf,
    /// Source roots that route a PR's (non-ignored) changed paths to `file`.
    pub(crate) roots: Vec<PathBuf>,
}

/// Configuration for a changelog-generation run.
///
/// Build one with [`Config::new`], optionally add extra per-crate changelogs with
/// [`Config::changelog`] and override the ignored paths with [`Config::ignore_paths`], then call
/// [`Config::run`].
#[derive(Debug, Clone)]
pub struct Config {
    /// The repository in `owner/name` form.
    pub(crate) repo: String,
    /// The repo-root-relative path to the primary (catch-all) changelog file.
    pub(crate) default_changelog: PathBuf,
    /// Additional registered changelog targets.
    pub(crate) changelogs: Vec<ChangelogTarget>,
    /// Repo-root-relative paths ignored for routing purposes.
    pub(crate) ignore_paths: Vec<PathBuf>,
}

impl Config {
    /// Creates a new configuration.
    ///
    /// `repo` is the GitHub repository in `owner/name` form. `default_changelog` is the
    /// repo-root-relative path to the primary changelog file: it is required, since it is both
    /// the catch-all destination (used when no more specific changelog matches a PR) and the
    /// home of the `git-changelog:last-commit` marker.
    pub fn new(repo: impl Into<String>, default_changelog: impl Into<PathBuf>) -> Self {
        Self {
            repo: repo.into(),
            default_changelog: default_changelog.into(),
            changelogs: Vec::new(),
            ignore_paths: default_ignore_paths(),
        }
    }

    /// Registers an additional changelog file, routed to by any of the given source roots.
    ///
    /// A PR routes to this changelog if any of its (non-ignored) changed paths starts with one
    /// of `roots`. Roots and the file path are repo-root-relative.
    #[must_use]
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

    /// Overrides the repo-root-relative paths ignored for routing purposes.
    ///
    /// Defaults to `["Cargo.toml", "Cargo.lock"]`. Paths are matched exactly against a PR's
    /// changed paths (repo-root-relative) -- a nested `crate/Cargo.toml` is *not* ignored by the
    /// default `Cargo.toml` entry.
    #[must_use]
    pub fn ignore_paths<R, P>(mut self, paths: R) -> Self
    where
        R: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.ignore_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Runs the full pipeline: `collect` then `apply`.
    pub fn run(&self, args: Args) -> Result<()> {
        let collected = collect(self, &args)?;
        apply(self, &collected)
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
