//! The markdown-editing core (pure): merging new entries into `## [Unreleased]`, and inserting
//! the reference-style link definitions they need.

use anyhow::{Context as _, Result, bail};
use std::collections::{BTreeMap, BTreeSet};

// --- Unreleased-section merge -------------------------------------------------------------

const CANONICAL_SECTIONS: [&str; 6] = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

fn canonical_rank(name: &str) -> usize {
    CANONICAL_SECTIONS.iter().position(|s| *s == name).unwrap_or(usize::MAX)
}

#[derive(Debug, Clone)]
enum BulletLine {
    Prose(String),
    Bullet { pr: Option<u64>, text: String },
}

fn classify_line(line: &str) -> BulletLine {
    let trimmed = line.trim_start();
    if let Some(stripped) = trimmed.strip_prefix("- ") {
        BulletLine::Bullet {
            pr: parse_bullet_pr(stripped),
            text: line.to_owned(),
        }
    } else {
        BulletLine::Prose(line.to_owned())
    }
}

fn parse_bullet_pr(bullet_text: &str) -> Option<u64> {
    let idx = bullet_text.find("([#")?;
    let rest = &bullet_text[idx + 3..];
    let end = rest.find(']')?;
    rest[..end].parse().ok()
}

fn bullet_line_text(line: &BulletLine) -> &str {
    match line {
        BulletLine::Prose(s) | BulletLine::Bullet { text: s, .. } => s,
    }
}

/// Inserts a new bullet line into `items`, keeping ascending PR-number order among the existing
/// `Bullet` entries. New bullets with no comparable existing bullets land right after the last
/// existing bullet (or at the very end, if there are none).
fn insert_bullet(items: &mut Vec<BulletLine>, pr: u64, line: String) {
    let mut insert_at = None;
    let mut last_bullet_idx = None;
    for (idx, item) in items.iter().enumerate() {
        if let BulletLine::Bullet { pr: existing_pr, .. } = item {
            last_bullet_idx = Some(idx);
            if existing_pr.is_some_and(|ep| ep > pr) {
                insert_at = Some(idx);
                break;
            }
        }
    }
    let at = insert_at.unwrap_or_else(|| last_bullet_idx.map_or(items.len(), |i| i + 1));
    items.insert(at, BulletLine::Bullet { pr: Some(pr), text: line });
}

#[derive(Debug, Clone)]
struct Subsection {
    name: String,
    heading: String,
    body_lines: Vec<BulletLine>,
}

impl Subsection {
    fn render(&self) -> Vec<String> {
        let mut out = vec![self.heading.clone()];
        out.extend(self.body_lines.iter().map(|l| bullet_line_text(l).to_owned()));
        out
    }
}

fn is_ref_def_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let Some(idx) = rest.find(']') else {
        return false;
    };
    rest[idx + 1..].starts_with(':')
}

fn parse_subsections(rest: &[&str]) -> Vec<Subsection> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i].trim_start().starts_with("### ") {
            let heading = (*rest[i]).to_owned();
            let name = heading.trim_start().trim_start_matches("### ").trim().to_owned();
            let mut j = i + 1;
            while j < rest.len() && !rest[j].trim_start().starts_with("### ") {
                j += 1;
            }
            let body_lines = rest[i + 1..j].iter().map(|l| classify_line(l)).collect();
            out.push(Subsection { name, heading, body_lines });
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn section_insert_position(subsections: &[Subsection], name: &str) -> usize {
    let rank = canonical_rank(name);
    subsections
        .iter()
        .position(|s| canonical_rank(&s.name) > rank)
        .unwrap_or(subsections.len())
}

fn insert_section_entries(subsections: &mut Vec<Subsection>, name: &str, entries: &[(u64, String)]) {
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by_key(|(pr, _)| *pr);
    if let Some(sub) = subsections.iter_mut().find(|s| s.name == name) {
        for (pr, text) in &sorted_entries {
            insert_bullet(&mut sub.body_lines, *pr, format!("- {text}"));
        }
    } else {
        let mut body_lines = vec![BulletLine::Prose(String::new())];
        for (pr, text) in &sorted_entries {
            body_lines.push(BulletLine::Bullet {
                pr: Some(*pr),
                text: format!("- {text}"),
            });
        }
        let heading = format!("### {name}");
        let pos = section_insert_position(subsections, name);
        subsections.insert(
            pos,
            Subsection {
                name: name.to_owned(),
                heading,
                body_lines,
            },
        );
    }
}

fn splice_bullets(prelude: &[&str], new_entries: &[(u64, String)]) -> Vec<String> {
    let mut items: Vec<BulletLine> = prelude.iter().map(|l| classify_line(l)).collect();
    let mut sorted_entries = new_entries.to_vec();
    sorted_entries.sort_by_key(|(pr, _)| *pr);
    for (pr, text) in sorted_entries {
        insert_bullet(&mut items, pr, format!("- {text}"));
    }
    items.iter().map(|l| bullet_line_text(l).to_owned()).collect()
}

/// Merges new section entries and loose placeholder bullets into a changelog's
/// `## [Unreleased]` section, using sorted (ascending PR-number) insertion. Existing entries and
/// their exact formatting are left untouched; only whole new lines are inserted.
pub(crate) fn merge_into_unreleased(
    content: &str,
    new_sections: &BTreeMap<String, Vec<(u64, String)>>,
    new_placeholders: &[(u64, String)],
) -> Result<String> {
    if new_sections.is_empty() && new_placeholders.is_empty() {
        return Ok(content.to_owned());
    }

    let lines: Vec<&str> = content.lines().collect();
    let unreleased_idx = lines
        .iter()
        .position(|l| l.trim() == "## [Unreleased]")
        .context("could not find a `## [Unreleased]` heading")?;

    let boundary_idx = lines[unreleased_idx + 1..]
        .iter()
        .position(|l| l.starts_with("## ") || is_ref_def_line(l))
        .map_or(lines.len(), |rel| unreleased_idx + 1 + rel);

    let body: &[&str] = &lines[unreleased_idx + 1..boundary_idx];
    let first_h3 = body.iter().position(|l| l.trim_start().starts_with("### "));
    let (prelude, rest) = body.split_at(first_h3.unwrap_or(body.len()));

    let new_prelude = splice_bullets(prelude, new_placeholders);

    let mut subsections = parse_subsections(rest);
    for (name, entries) in new_sections {
        insert_section_entries(&mut subsections, name, entries);
    }

    let mut new_body_lines: Vec<String> = new_prelude;
    if !subsections.is_empty() {
        if !new_body_lines.is_empty() && !new_body_lines.last().is_some_and(|l| l.is_empty()) {
            new_body_lines.push(String::new());
        }
        for (i, sub) in subsections.iter().enumerate() {
            if i > 0 {
                new_body_lines.push(String::new());
            }
            new_body_lines.extend(sub.render());
        }
    }
    if !new_body_lines.last().is_some_and(|l| l.is_empty()) {
        new_body_lines.push(String::new());
    }

    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len() + new_body_lines.len());
    out_lines.extend(lines[..=unreleased_idx].iter().map(|s| (*s).to_owned()));
    out_lines.extend(new_body_lines);
    out_lines.extend(lines[boundary_idx..].iter().map(|s| (*s).to_owned()));

    let mut out = out_lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

// --- Reference-definition block merge ------------------------------------------------------

fn def_label(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('[')?;
    let idx = rest.find(']')?;
    Some(&rest[..idx])
}

fn pr_label_num(line: &str) -> Option<u64> {
    def_label(line)?.strip_prefix('#')?.parse().ok()
}

fn author_label(line: &str) -> Option<String> {
    def_label(line)?.strip_prefix('@').map(str::to_owned)
}

/// Index of the first line of the trailing reference-definition block: the maximal run of blank
/// and definition lines at the end of the file. Definition groups within it are separated by blank
/// lines with no real content between them, so this captures every group but stops at the last
/// line of actual changelog content. New definitions are always placed within this block.
fn trailing_block_start(lines: &[String]) -> usize {
    let mut start = lines.len();
    while start > 0 {
        let line = &lines[start - 1];
        if line.trim().is_empty() || is_ref_def_line(line) {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

/// Appends `new_line` as the start of a fresh definition group at the very bottom of the file (the
/// end of the trailing block), separated from whatever precedes it by a single blank line.
fn append_new_group(lines: &mut Vec<String>, new_line: String) {
    if lines.last().is_some_and(|l| !l.trim().is_empty()) {
        lines.push(String::new());
    }
    lines.push(new_line);
}

/// Inserts a `[#N]` definition line into the trailing definition block, keeping ascending
/// PR-number order among the block's existing `[#N]` definitions. Definitions elsewhere in the
/// file are ignored for placement (and, like everything else, never moved or removed). If the
/// block has no `[#N]` definitions yet, a new group is appended at the end.
fn insert_pr_def_line(lines: &mut Vec<String>, n: u64, new_line: String) {
    let block_start = trailing_block_start(lines);
    let pr_indices: Vec<usize> = (block_start..lines.len())
        .filter(|&i| pr_label_num(&lines[i]).is_some())
        .collect();
    let Some(&last) = pr_indices.last() else {
        append_new_group(lines, new_line);
        return;
    };
    let at = pr_indices
        .iter()
        .find(|&&i| pr_label_num(&lines[i]).is_some_and(|existing| existing > n))
        .copied()
        .unwrap_or(last + 1);
    lines.insert(at, new_line);
}

/// Inserts a `[@login]` definition line into the trailing definition block, keeping alphabetical
/// order among the block's existing `[@login]` definitions. Definitions elsewhere in the file are
/// ignored for placement (and never moved or removed). If the block has no `[@login]` definitions
/// yet, a new group is started just before the block's `[#N]` group (if any) or appended at the end.
fn insert_author_def_line(lines: &mut Vec<String>, login: &str, new_line: String) {
    let block_start = trailing_block_start(lines);
    let author_indices: Vec<usize> = (block_start..lines.len())
        .filter(|&i| author_label(&lines[i]).is_some())
        .collect();
    let Some(&last) = author_indices.last() else {
        if let Some(first_pr) = (block_start..lines.len()).find(|&i| pr_label_num(&lines[i]).is_some()) {
            lines.insert(first_pr, String::new());
            lines.insert(first_pr, new_line);
        } else {
            append_new_group(lines, new_line);
        }
        return;
    };
    let at = author_indices
        .iter()
        .find(|&&i| author_label(&lines[i]).is_some_and(|existing| existing.as_str() > login))
        .copied()
        .unwrap_or(last + 1);
    lines.insert(at, new_line);
}

/// Ensures a `[#N]: <repo>/pull/N` definition exists for every `pr_numbers` entry, and a
/// `[@u]: https://github.com/u` definition for every `authors` entry.
///
/// This only ever *inserts* new definition lines (deduplicated, sorted) among the existing ones:
/// existing definitions -- including `[MSRV]` and version-compare (`[Unreleased]`/`[x.y.z]`)
/// links -- are never moved or removed. As a safety net for that invariant, it bails if any
/// existing reference definition would be lost.
pub(crate) fn update_reference_defs(
    content: &str,
    repo: &str,
    pr_numbers: &BTreeSet<u64>,
    authors: &BTreeSet<String>,
) -> Result<String> {
    if pr_numbers.is_empty() && authors.is_empty() {
        return Ok(content.to_owned());
    }

    let mut lines: Vec<String> = content.lines().map(ToOwned::to_owned).collect();

    let existing_prs: BTreeSet<u64> = lines.iter().filter_map(|l| pr_label_num(l)).collect();
    let existing_authors: BTreeSet<String> = lines.iter().filter_map(|l| author_label(l)).collect();

    // Definitions we started with, so we can verify none are dropped by the insertions below.
    let original_defs: Vec<String> = lines.iter().filter(|l| is_ref_def_line(l)).cloned().collect();

    for author in authors {
        if !existing_authors.contains(author) {
            insert_author_def_line(&mut lines, author, format!("[@{author}]: https://github.com/{author}"));
        }
    }
    for &n in pr_numbers {
        if !existing_prs.contains(&n) {
            insert_pr_def_line(&mut lines, n, format!("[#{n}]: https://github.com/{repo}/pull/{n}"));
        }
    }

    // Invariant: we only insert, never delete. Bail rather than write a lossy changelog.
    let final_defs: BTreeSet<&String> = lines.iter().filter(|l| is_ref_def_line(l)).collect();
    for def in &original_defs {
        if !final_defs.contains(def) {
            bail!("refusing to drop existing reference definition `{def}` while updating the changelog");
        }
    }

    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Collected, Config, PrData};
    use std::path::{Path, PathBuf};

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
    fn merge_sorted_insertion_with_straggler() {
        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), changelog_fixture());

        // PR #150 is a straggler that should land between #100 and #200.
        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![PrData {
                number: 150,
                title: "Straggler PR".to_owned(),
                body: Some("**Changelog**\n\n> ### Added\n>\n> - Straggler entry.\n".to_owned()),
                author: "carol".to_owned(),
                co_authors: Vec::new(),
                changed_paths: vec![PathBuf::from("src/lib.rs")],
            }],
        };

        let output = crate::process::process(&collected, &cfg, &current).expect("process should succeed");
        let new_content = output.updated.get(Path::new("CHANGELOG.md")).expect("file changed");

        let added_idx = new_content.find("### Added").unwrap();
        let idx100 = new_content.find("Existing entry one").unwrap();
        let idx150 = new_content.find("Straggler entry").unwrap();
        let idx200 = new_content.find("Existing entry two").unwrap();
        assert!(added_idx < idx100 && idx100 < idx150 && idx150 < idx200, "expected ascending PR order");
        assert!(new_content.contains("([#150][] by [@carol][])"));

        // Existing entries are untouched.
        assert!(new_content.contains("- Existing entry one. ([#100][] by [@alice][])"));
        assert!(new_content.contains("- Existing entry two. ([#200][] by [@bob][])"));
    }

    #[test]
    fn merge_creates_subsection_in_canonical_order() {
        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), changelog_fixture());

        // File has only "### Added"; adding a "Fixed" entry should create "### Fixed" *after*
        // "### Added" (canonical order), and a "Changed" entry should land *between* them.
        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![
                PrData {
                    number: 300,
                    title: "Fix something".to_owned(),
                    body: Some("**Changelog**\n\n> ### Fixed\n>\n> - Fixed something.\n".to_owned()),
                    author: "dave".to_owned(),
                    co_authors: Vec::new(),
                    changed_paths: vec![PathBuf::from("src/lib.rs")],
                },
                PrData {
                    number: 250,
                    title: "Change something".to_owned(),
                    body: Some("**Changelog**\n\n> ### Changed\n>\n> - Changed something.\n".to_owned()),
                    author: "erin".to_owned(),
                    co_authors: Vec::new(),
                    changed_paths: vec![PathBuf::from("src/lib.rs")],
                },
            ],
        };

        let output = crate::process::process(&collected, &cfg, &current).expect("process should succeed");
        let new_content = output.updated.get(Path::new("CHANGELOG.md")).expect("file changed");

        let added_idx = new_content.find("### Added").unwrap();
        let changed_idx = new_content.find("### Changed").unwrap();
        let fixed_idx = new_content.find("### Fixed").unwrap();
        assert!(added_idx < changed_idx && changed_idx < fixed_idx, "expected canonical section order");
    }

    #[test]
    fn merge_placeholder_is_loose_bullet_before_first_subsection() {
        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), changelog_fixture());

        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![PrData {
                number: 50,
                title: "Some internal refactor".to_owned(),
                body: None,
                author: "frank".to_owned(),
                co_authors: Vec::new(),
                changed_paths: vec![PathBuf::from("src/lib.rs")],
            }],
        };

        let output = crate::process::process(&collected, &cfg, &current).expect("process should succeed");
        let new_content = output.updated.get(Path::new("CHANGELOG.md")).expect("file changed");

        let msrv_idx = new_content.find("This release has an [MSRV]").unwrap();
        let placeholder_idx = new_content.find("no-changelog: Some internal refactor").unwrap();
        let added_idx = new_content.find("### Added").unwrap();
        assert!(msrv_idx < placeholder_idx && placeholder_idx < added_idx);
        assert!(new_content.contains("([#50][] by [@frank][])"));
    }

    // --- Reference defs ------------------------------------------------------------------------

    #[test]
    fn reference_defs_created_deduped_and_sorted() {
        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), changelog_fixture());

        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![
                PrData {
                    number: 150,
                    title: "First".to_owned(),
                    body: Some("**Changelog**\n\n> ### Added\n>\n> - Entry one.\n".to_owned()),
                    author: "alice".to_owned(), // duplicate of an existing author
                    co_authors: Vec::new(),
                    changed_paths: vec![PathBuf::from("src/lib.rs")],
                },
                PrData {
                    number: 50,
                    title: "Second".to_owned(),
                    body: Some("**Changelog**\n\n> ### Added\n>\n> - Entry two.\n".to_owned()),
                    author: "zed".to_owned(),
                    co_authors: vec!["amy".to_owned()],
                    changed_paths: vec![PathBuf::from("src/lib.rs")],
                },
            ],
        };

        let output = crate::process::process(&collected, &cfg, &current).expect("process should succeed");
        let new_content = output.updated.get(Path::new("CHANGELOG.md")).expect("file changed");

        // PR defs: numeric ascending, existing untouched, new ones inserted, no duplicates.
        let pr_defs: Vec<&str> = new_content.lines().filter(|l| l.starts_with("[#")).collect();
        assert_eq!(
            pr_defs,
            vec![
                "[#50]: https://github.com/owner/repo/pull/50",
                "[#100]: https://example.com/pull/100",
                "[#150]: https://github.com/owner/repo/pull/150",
                "[#200]: https://example.com/pull/200",
            ]
        );

        // Author defs: alphabetical, existing untouched, deduped (alice already existed), amy/zed added.
        let author_defs: Vec<&str> = new_content.lines().filter(|l| l.starts_with("[@")).collect();
        assert_eq!(
            author_defs,
            vec![
                "[@alice]: https://github.com/alice",
                "[@amy]: https://github.com/amy",
                "[@bob]: https://github.com/bob",
                "[@zed]: https://github.com/zed",
            ]
        );

        // MSRV / version-compare links untouched.
        assert!(new_content.contains("[MSRV]: README.md#minimum-supported-rust-version-msrv"));
        assert!(new_content.contains("[Unreleased]: https://example.com/compare/v0.1.0...HEAD"));
        assert!(new_content.contains("[0.1.0]: https://example.com/compare/v0.0.0...v0.1.0"));
    }

    #[test]
    fn reference_defs_created_from_scratch_when_absent() {
        // A brand-new changelog (like examples/test-package/CHANGELOG.md) has no `[#N]` or
        // `[@u]` groups yet.
        let content = "\
# Changelog

## [Unreleased]

This release has an [MSRV][] of 1.88.

This package was created.

[MSRV]: README.md#minimum-supported-rust-version-msrv

[Unreleased]: https://example.com/compare/v0.1.0...HEAD
"
        .to_owned();

        let cfg = Config::new("owner/repo", "CHANGELOG.md");
        let mut current = BTreeMap::new();
        current.insert(PathBuf::from("CHANGELOG.md"), content);

        let collected = Collected {
            head_sha: "deadbeef".to_owned(),
            prs: vec![PrData {
                number: 1,
                title: "First PR".to_owned(),
                body: Some("**Changelog**\n\n> ### Added\n>\n> - Something.\n".to_owned()),
                author: "alice".to_owned(),
                co_authors: Vec::new(),
                changed_paths: vec![PathBuf::from("src/lib.rs")],
            }],
        };

        let output = crate::process::process(&collected, &cfg, &current).expect("process should succeed");
        let new_content = output.updated.get(Path::new("CHANGELOG.md")).expect("file changed");
        assert!(new_content.contains("[@alice]: https://github.com/alice"));
        assert!(new_content.contains("[#1]: https://github.com/owner/repo/pull/1"));
    }

    // --- Reference defs: trailing-content preservation (regression) ----------------------------

    #[test]
    fn reference_defs_preserve_trailing_non_definition_content() {
        // A definition is followed by prose. Rebuilding the def block must NOT drop that prose.
        let content = "\
# Changelog

## [Unreleased]

### Added

- Thing. ([#5][] by [@x][])

[#5]: https://example.com/pull/5

A closing note that is not a definition.
";
        let mut pr_numbers = BTreeSet::new();
        pr_numbers.insert(7_u64);
        let authors = BTreeSet::new();
        let out = update_reference_defs(content, "owner/repo", &pr_numbers, &authors)
            .expect("update should succeed");

        assert!(
            out.contains("A closing note that is not a definition."),
            "trailing prose must be preserved, not dropped"
        );
        assert!(out.contains("[#5]: https://example.com/pull/5"), "existing def preserved");
        assert!(out.contains("[#7]: https://github.com/owner/repo/pull/7"), "new def added");
    }

    #[test]
    fn reference_defs_do_not_reorder_existing_groups() {
        // Non-canonical layout: the PR group appears BEFORE the author group. Insertion must
        // slot new defs into each group in place, never rewriting/swapping the existing order.
        let content = "\
# Changelog

## [Unreleased]

### Added

- A. ([#10][] by [@bob][])

[#10]: https://example.com/pull/10

[@bob]: https://github.com/bob
";
        let mut pr_numbers = BTreeSet::new();
        pr_numbers.insert(20_u64);
        let mut authors = BTreeSet::new();
        authors.insert("amy".to_owned());
        let out = update_reference_defs(content, "owner/repo", &pr_numbers, &authors)
            .expect("update should succeed");

        // Existing group order preserved: PR group still precedes the author group.
        let pr_10 = out.find("[#10]:").unwrap();
        let author_bob = out.find("[@bob]:").unwrap();
        assert!(pr_10 < author_bob, "existing group order must be preserved");

        // New defs land in their own groups, sorted.
        assert!(out.contains("[#20]: https://github.com/owner/repo/pull/20"));
        let author_amy = out.find("[@amy]:").unwrap();
        assert!(author_amy < author_bob, "amy sorts before bob within the author group");
    }

    #[test]
    fn reference_defs_target_bottom_block_ignoring_stray_defs() {
        // A stray `[#3]` definition sits above real content; the true definition block is at the
        // bottom. A new `[#8]` must join the BOTTOM block (after `[#5]`), not the stray `[#3]`.
        let content = "\
# Changelog

## [Unreleased]

### Added

- New. ([#8][] by [@z][])

[#3]: https://example.com/pull/3

Some prose that breaks the definition block.

[#5]: https://example.com/pull/5

[@z]: https://github.com/z
";
        let mut pr_numbers = BTreeSet::new();
        pr_numbers.insert(8_u64);
        let authors = BTreeSet::new();
        let out = update_reference_defs(content, "owner/repo", &pr_numbers, &authors)
            .expect("update should succeed");

        let stray_3 = out.find("[#3]:").unwrap();
        let prose = out.find("Some prose that breaks").unwrap();
        let bottom_5 = out.find("[#5]:").unwrap();
        let new_8 = out.find("[#8]:").unwrap();
        // The stray def and prose are preserved, and the new def is in the bottom block.
        assert!(stray_3 < prose, "stray [#3] preserved above the prose");
        assert!(prose < bottom_5, "bottom block sits after the prose");
        assert!(bottom_5 < new_8, "new [#8] joins the bottom block, after [#5]");
    }
}
