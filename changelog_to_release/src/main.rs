// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Some basic release engineering.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::collections::HashSet;
use std::range::Range;

/// Gets the markdown (suitable for a GitHub release)
fn extract_version_section(input: &str, version: &str) -> Option<String> {
    // Collect reference definition source spans before consuming the parser.
    let parser = Parser::new(input);
    let ref_defs: Vec<(String, Range<usize>)> = parser
        .reference_definitions()
        .iter()
        .map(|(label, def)| (label.to_string(), def.span.clone().into()))
        .collect();

    let mut output = String::new();
    let mut rewrites: Vec<(Range<usize>, String)> = Vec::new();

    let mut in_section = false;
    let mut in_header = false;
    let mut found_section = false;
    let mut section_range = 0..input.len();
    let mut used_labels: HashSet<String> = HashSet::new();
    let mut in_rewritten_link = false;
    let mut next_event_ends_rewrite = false;

    for (event, range) in parser.into_offset_iter() {
        // Convert to new-style range.
        let range: Range<usize> = range.into();

        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => {
                if input[range].contains(version) {
                    if found_section {
                        // TODO: Return Err(Error::MultipleMatchingSections);
                        return None;
                    }
                    found_section = true;
                    section_range.start = range.end;
                    in_header = true;
                } else {
                    if in_section {
                        section_range.end = range.start;
                    }
                    in_section = false;
                    // We keep going so that we can check there weren't two matching headers.
                }
            }
            // The header contains arbitrary markdown. Skip past it.
            Event::End(TagEnd::Heading { .. }) if in_header => {
                in_header = false;
                in_section = true;
            }
            Event::Start(Tag::Link {
                link_type: _,
                dest_url,
                title: _,
                id,
            }) if in_section => {
                // In theory, the link can contain arbitrary markdown, so cmark-pulldown pull-parses it.
                // However, we are only rewriting the GitHub URLs, so we don't actually care about that inner markdown,
                // so we can use a fragile hack.
                // Get the actual full title text. We only are rewriting the GitHub URLs, so it doesn't matter that this is fragile.
                let Some((link_title, _)) = input[range]
                    .strip_prefix('[')
                    .and_then(|it| it.split_once(']'))
                else {
                    continue;
                };
                if let Some(username) = link_title.trim().strip_prefix("@")
                    && dest_url.to_lowercase().ends_with(&username.to_lowercase())
                {
                    rewrites.push((range, format!("@{username}")));
                    in_rewritten_link = true;
                } else if let Some(issue) = link_title.trim().strip_prefix("#")
                    && issue.chars().all(|it| it.is_ascii_digit())
                    && dest_url.ends_with(&issue)
                {
                    in_rewritten_link = true;
                    rewrites.push((range, format!("#{issue}")));
                } else if !id.is_empty() {
                    used_labels.insert(id.into());
                }
            }
            Event::End(TagEnd::Link) if in_rewritten_link => {
                next_event_ends_rewrite = true;
                in_rewritten_link = false;
                // Intentionally do nothing.
            }
            _ if next_event_ends_rewrite => {
                rewrites.last_mut().unwrap().0.end = range.start;
                next_event_ends_rewrite = false;
            }
            _ => {}
        }
    }
    if !found_section {
        return None;
    }
    for (_, range) in &ref_defs {
        rewrites.push((*range, String::new()));
    }
    rewrites.sort_by_key(|(range, _)| range.start);
    let mut source_cursor = section_range.start;
    let mut started = false;
    for (rewrite_range, rewrite) in rewrites {
        if rewrite_range.start < source_cursor {
            assert!(!started, "Overlapping ranges should be impossible.");
            debug_assert!(
                rewrite_range.end < source_cursor,
                "Overlapping ranges should be impossible."
            );
            continue;
        }
        started = true;
        if rewrite_range.start > section_range.end {
            break;
        }
        output.push_str(&input[source_cursor..rewrite_range.start]);
        output.push_str(&rewrite);
        source_cursor = rewrite_range.end;
    }
    output.push_str(&input[source_cursor..section_range.end]);

    // Clear any trailing newlines.
    output.truncate(output.trim_end().len());

    let needed_defs: Vec<_> = ref_defs
        .iter()
        .filter(|(label, _): &&(String, Range<usize>)| used_labels.contains(label.as_str()))
        .collect();

    if !needed_defs.is_empty() {
        output.push_str("\n\n");
        for (_, span) in needed_defs {
            output.push_str(input[span.start..span.end].trim_end());
            output.push('\n');
        }
    } else {
        output.push('\n');
    }

    Some(output)
}

fn main() {
    let version = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: release_eng <version>");
        std::process::exit(1);
    });

    let input = include_str!("../../CHANGELOG.md");

    match extract_version_section(input, &version) {
        Some(section) => print!("{}", section.trim()),
        None => {
            eprintln!("Version '{}' not found in changelog", version);
            std::process::exit(1);
        }
    }
}
