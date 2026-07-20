// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stage 2: process (pure).
//!
//! Extracts, routes, and merges -- no I/O at all.

use crate::extract::{Extraction, extract_changelog};
use crate::merge::{merge_into_unreleased, update_reference_defs};
use crate::{Collected, Config, PrData, ProcessOutput, Summary};
use anyhow::{Result, anyhow};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The pure processing stage: extracts, routes, and merges.
///
/// Takes the impure [`mod@crate::collect`] output plus the *current* contents of every target
/// changelog file, keyed by repo-root-relative path, and returns the new contents for every file
/// that changed. Does no I/O whatsoever.
pub(crate) fn process(
    collected: &Collected,
    config: &Config,
    current: &BTreeMap<PathBuf, String>,
) -> Result<ProcessOutput> {
    let mut per_file: BTreeMap<PathBuf, FileAccumulator> = BTreeMap::new();
    let mut summary = Summary::default();

    for pr in &collected.prs {
        let targets = route(config, &pr.changed_paths);
        match extract_changelog(pr.body.as_deref()) {
            Extraction::NoneOptOut => {
                summary.none_skipped += 1;
            }
            Extraction::Sections(sections) => {
                let logins = attribution_logins(pr);
                let attribution = format_attribution(&logins);
                for target in &targets {
                    let accum = per_file.entry(target.clone()).or_default();
                    for section in &sections {
                        for bullet in &section.bullets {
                            let text = format!("{bullet} ([#{}][] by {attribution})", pr.number);
                            accum
                                .sections
                                .entry(section.name.clone())
                                .or_default()
                                .push((pr.number, text));
                            *summary.entries_added.entry(target.clone()).or_default() += 1;
                        }
                    }
                    accum.pr_numbers.insert(pr.number);
                    accum.authors.extend(logins.iter().cloned());
                }
            }
            Extraction::Placeholder => {
                let logins = attribution_logins(pr);
                let attribution = format_attribution(&logins);
                let text = format!(
                    "no-changelog: {} ([#{}][] by {attribution})",
                    pr.title, pr.number
                );
                for target in &targets {
                    let accum = per_file.entry(target.clone()).or_default();
                    accum.placeholders.push((pr.number, text.clone()));
                    accum.pr_numbers.insert(pr.number);
                    accum.authors.extend(logins.iter().cloned());
                    summary.placeholders += 1;
                    *summary.entries_added.entry(target.clone()).or_default() += 1;
                }
            }
        }
    }

    let mut updated = BTreeMap::new();
    for (file, accum) in per_file {
        let content = current.get(&file).ok_or_else(|| {
            anyhow!(
                "no current content provided for changelog file {}",
                file.display()
            )
        })?;
        let merged = merge_into_unreleased(content, &accum.sections, &accum.placeholders)?;
        let with_refs =
            update_reference_defs(&merged, &config.repo, &accum.pr_numbers, &accum.authors)?;
        updated.insert(file, with_refs);
    }

    Ok(ProcessOutput { updated, summary })
}

#[derive(Debug, Default)]
struct FileAccumulator {
    sections: BTreeMap<String, Vec<(u64, String)>>,
    placeholders: Vec<(u64, String)>,
    pr_numbers: BTreeSet<u64>,
    authors: BTreeSet<String>,
}

/// Determines which changelog files a PR's changed paths route to.
///
/// A changelog matches if any of its roots contains a non-ignored changed path. If no
/// registered changelog matches (typically because every changed path is ignored), the PR falls
/// back to `config`'s default changelog.
fn route(config: &Config, changed_paths: &[PathBuf]) -> BTreeSet<PathBuf> {
    let non_ignored: Vec<&Path> = changed_paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|p| {
            !config
                .ignore_paths
                .iter()
                .any(|ignored| ignored.as_path() == *p)
        })
        .collect();

    let mut matched: BTreeSet<PathBuf> = config
        .changelogs
        .iter()
        .filter(|target| {
            target
                .roots
                .iter()
                .any(|root| non_ignored.iter().any(|p| p.starts_with(root)))
        })
        .map(|target| target.file.clone())
        .collect();

    if matched.is_empty() {
        matched.insert(config.default_changelog.clone());
    }
    matched
}

/// Collects the deduplicated, order-preserving list of attributed logins (author first, then
/// co-authors) for a PR.
fn attribution_logins(pr: &PrData) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut logins = Vec::new();
    for login in std::iter::once(pr.author.clone()).chain(pr.co_authors.iter().cloned()) {
        if seen.insert(login.clone()) {
            logins.push(login);
        }
    }
    logins
}

/// Renders `by [@a][] and [@b][]`-style attribution (without the leading `by `).
fn format_attribution(logins: &[String]) -> String {
    match logins {
        [] => String::new(),
        [a] => format!("[@{a}][]"),
        [a, b] => format!("[@{a}][] and [@{b}][]"),
        many => {
            let (last, rest) = many.split_last().expect("non-empty slice");
            let joined = rest
                .iter()
                .map(|l| format!("[@{l}][]"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{joined}, and [@{last}][]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::new("linebender/vello", "CHANGELOG.md")
            .changelog(
                "sparse_strips/vello_cpu/CHANGELOG.md",
                ["sparse_strips/vello_cpu"],
            )
            .changelog(
                "sparse_strips/vello_common/CHANGELOG.md",
                [
                    "sparse_strips/vello_common",
                    "sparse_strips/vello_common/crate",
                ],
            )
    }

    fn pr(number: u64, author: &str, changed_paths: &[&str]) -> PrData {
        PrData {
            number,
            title: format!("Title for #{number}"),
            body: None,
            author: author.to_owned(),
            co_authors: Vec::new(),
            changed_paths: changed_paths.iter().map(PathBuf::from).collect(),
        }
    }

    // --- Routing -----------------------------------------------------------------------------

    #[test]
    fn routing_multi_root_pr_routes_to_multiple_files() {
        let cfg = config();
        let targets = route(
            &cfg,
            &[
                PathBuf::from("sparse_strips/vello_cpu/src/lib.rs"),
                PathBuf::from("sparse_strips/vello_common/src/lib.rs"),
            ],
        );
        assert_eq!(
            targets,
            BTreeSet::from([
                PathBuf::from("sparse_strips/vello_cpu/CHANGELOG.md"),
                PathBuf::from("sparse_strips/vello_common/CHANGELOG.md"),
            ])
        );
    }

    #[test]
    fn routing_ignored_root_only_falls_back_to_default() {
        let cfg = config();
        let targets = route(&cfg, &[PathBuf::from("Cargo.lock")]);
        assert_eq!(targets, BTreeSet::from([PathBuf::from("CHANGELOG.md")]));
    }

    #[test]
    fn routing_nested_cargo_toml_is_not_ignored() {
        let cfg = config();
        // A nested Cargo.toml is NOT matched by the exact "Cargo.toml" ignore entry, so it
        // routes normally to its crate's changelog.
        let targets = route(&cfg, &[PathBuf::from("sparse_strips/vello_cpu/Cargo.toml")]);
        assert_eq!(
            targets,
            BTreeSet::from([PathBuf::from("sparse_strips/vello_cpu/CHANGELOG.md")])
        );
    }

    // --- process() end-to-end summary -----------------------------------------------------------

    fn changelog_fixture() -> String {
        "\
# Changelog

## [Unreleased]

This release has an [MSRV][] of 1.88.

### Added

- Existing entry one. ([#100][] by [@alice][])
- Existing entry two. ([#200][] by [@bob][])

## [0.1.0][] - 2026-01-01

Initial release.

[MSRV]: README.md#minimum-supported-rust-version-msrv

[Unreleased]: https://example.com/compare/v0.1.0...HEAD
[0.1.0]: https://example.com/compare/v0.0.0...v0.1.0

[@alice]: https://github.com/alice
[@bob]: https://github.com/bob

[#100]: https://example.com/pull/100
[#200]: https://example.com/pull/200
"
        .to_owned()
    }

    #[test]
    fn process_reports_none_skips_and_placeholder_counts() {
        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), changelog_fixture());

        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![pr(400, "alice", &["src/lib.rs"]), {
                let mut p = pr(401, "bob", &["src/lib.rs"]);
                p.body = Some("Changelog: None".to_owned());
                p
            }],
        };

        let output = process(&collected, &cfg, &current).expect("process should succeed");
        assert_eq!(output.summary.none_skipped, 1);
        assert_eq!(output.summary.placeholders, 1);
    }
}
