// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stage 3: apply (impure).

use crate::collect::run_git;
use crate::process::process;
use crate::{Collected, Config, Summary};
use anyhow::{Context as _, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const MARKER_PREFIX: &str = "<!-- git-changelog:last-commit ";
const MARKER_SUFFIX: &str = " -->";

/// Reads the `git-changelog:last-commit` marker value out of changelog content, if present.
///
/// Shared with [`mod@crate::collect`], which uses it to find the base commit for a run.
pub(crate) fn read_marker(content: &str) -> Option<String> {
    content.lines().find_map(|l| {
        let t = l.trim();
        t.strip_prefix(MARKER_PREFIX)?
            .strip_suffix(MARKER_SUFFIX)
            .map(str::to_owned)
    })
}

/// Inserts or updates the `git-changelog:last-commit` marker, placing it directly under the
/// first `# Changelog` heading (i.e. outside `## [Unreleased]`, so it is never promoted into a
/// versioned section).
fn set_marker(content: &str, sha: &str) -> String {
    let marker_line = format!("{MARKER_PREFIX}{sha}{MARKER_SUFFIX}");
    let lines: Vec<&str> = content.lines().collect();

    let existing_idx = lines
        .iter()
        .position(|l| l.trim().starts_with(MARKER_PREFIX) && l.trim().ends_with(MARKER_SUFFIX));

    let out_lines: Vec<String> = if let Some(idx) = existing_idx {
        let mut out: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
        out[idx] = marker_line;
        out
    } else if let Some(idx) = lines.iter().position(|l| l.trim() == "# Changelog") {
        let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
        out.extend(lines[..=idx].iter().map(|s| (*s).to_owned()));
        out.push(marker_line);
        out.extend(lines[idx + 1..].iter().map(|s| (*s).to_owned()));
        out
    } else {
        let mut out = vec![marker_line];
        out.extend(lines.iter().map(|s| (*s).to_owned()));
        out
    };

    if out_lines.is_empty() {
        return String::new();
    }
    let mut out = out_lines.join("\n");
    if content.ends_with('\n') || existing_idx.is_none() {
        out.push('\n');
    }
    out
}

fn check_not_dirty(file: &Path) -> Result<()> {
    let path_str = file
        .to_str()
        .ok_or_else(|| anyhow!("non-UTF8 path: {}", file.display()))?;
    let status = run_git(&["status", "--porcelain", "--", path_str])
        .with_context(|| format!("failed to check git status of {}", file.display()))?;
    for line in status.lines() {
        let bytes = line.as_bytes();
        if bytes.len() >= 2 && bytes[1] != b' ' {
            bail!(
                "{} has unstaged working-tree modifications; commit, stash, or stage them before running git-changelog",
                file.display()
            );
        }
    }
    Ok(())
}

/// The impure apply stage: pre-flight dirty check, read files, call [`crate::process`], write
/// results, advance the marker last, and print an end-of-run summary to stderr.
pub(crate) fn apply(config: &Config, collected: &Collected) -> Result<()> {
    let files = config.all_changelog_files();

    for file in &files {
        check_not_dirty(file)?;
    }

    let mut current = BTreeMap::new();
    for file in &files {
        let content = fs::read_to_string(file).with_context(|| {
            format!(
                "missing target changelog file {} -- create it before running git-changelog",
                file.display()
            )
        })?;
        current.insert(file.clone(), content);
    }

    let output = process(collected, config, &current)?;

    for (file, content) in &output.updated {
        fs::write(file, content).with_context(|| format!("failed to write {}", file.display()))?;
    }

    let default_path = &config.default_changelog;
    let default_content = output
        .updated
        .get(default_path)
        .or_else(|| current.get(default_path))
        .cloned()
        .unwrap_or_default();
    let with_marker = set_marker(&default_content, &collected.head_sha);
    fs::write(default_path, with_marker)
        .with_context(|| format!("failed to write {}", default_path.display()))?;

    print_summary(&output.summary);
    Ok(())
}

fn print_summary(summary: &Summary) {
    eprintln!("git-changelog summary:");
    for (file, count) in &summary.entries_added {
        eprintln!(
            "  {}: {count} entr{}",
            file.display(),
            if *count == 1 { "y" } else { "ies" }
        );
    }
    eprintln!("  placeholders: {}", summary.placeholders);
    eprintln!("  skipped (Changelog: None): {}", summary.none_skipped);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_inserted_under_heading_outside_unreleased() {
        let content = "# Changelog\n\n## [Unreleased]\n\nSome prose.\n";
        let updated = set_marker(content, "abc123");
        let lines: Vec<&str> = updated.lines().collect();
        assert_eq!(lines[0], "# Changelog");
        assert_eq!(lines[1], "<!-- git-changelog:last-commit abc123 -->");
        // Still outside `## [Unreleased]`.
        let marker_idx = lines
            .iter()
            .position(|l| l.contains("git-changelog:last-commit"))
            .unwrap();
        let unreleased_idx = lines
            .iter()
            .position(|l| l.trim() == "## [Unreleased]")
            .unwrap();
        assert!(marker_idx < unreleased_idx);
    }

    #[test]
    fn marker_updates_existing_value() {
        let content = "# Changelog\n<!-- git-changelog:last-commit old -->\n\n## [Unreleased]\n";
        let updated = set_marker(content, "new123");
        assert!(updated.contains("<!-- git-changelog:last-commit new123 -->"));
        assert!(!updated.contains("old"));
    }
}
