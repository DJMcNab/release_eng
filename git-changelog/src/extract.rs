//! PR-body `Changelog` block parsing (pure).

/// The outcome of parsing a PR body's `Changelog` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Extraction {
    /// The PR opted out via `Changelog: None`.
    NoneOptOut,
    /// A recognizable `Changelog` marker and blockquote, parsed into sections.
    Sections(Vec<ChangelogSection>),
    /// No recognizable marker/blockquote (or an empty/malformed one).
    Placeholder,
}

/// A single `### Section` from a PR body's changelog blockquote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangelogSection {
    /// The section heading text (e.g. `Added`).
    pub(crate) name: String,
    /// The bullet lines under this section, with the leading `- ` stripped.
    pub(crate) bullets: Vec<String>,
}

/// Parses a PR body into an [`Extraction`].
///
/// Lenient: the `**Changelog**` marker's bold is optional, and only HTML comments are stripped
/// from the blockquote content -- everything else (including a `[Breaking change:]` bracket) is
/// kept verbatim.
pub(crate) fn extract_changelog(body: Option<&str>) -> Extraction {
    let Some(body) = body else {
        return Extraction::Placeholder;
    };
    let stripped = strip_html_comments(body);
    let lines: Vec<&str> = stripped.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        match normalize_marker_line(line) {
            Some(MarkerKind::None) => return Extraction::NoneOptOut,
            Some(MarkerKind::Changelog) => {
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                if j >= lines.len() || !lines[j].trim_start().starts_with('>') {
                    return Extraction::Placeholder;
                }
                let mut quote_lines = Vec::new();
                while j < lines.len() && lines[j].trim_start().starts_with('>') {
                    quote_lines.push(dequote(lines[j]));
                    j += 1;
                }
                let sections = parse_changelog_sections(&quote_lines);
                if sections.is_empty() || sections.iter().all(|s| s.bullets.is_empty()) {
                    return Extraction::Placeholder;
                }
                return Extraction::Sections(sections);
            }
            None => {}
        }
    }
    Extraction::Placeholder
}

enum MarkerKind {
    None,
    Changelog,
}

fn normalize_marker_line(line: &str) -> Option<MarkerKind> {
    let t = line.trim();
    let t = t.strip_prefix("**").and_then(|s| s.strip_suffix("**")).unwrap_or(t);
    let t = t.trim();
    if t.eq_ignore_ascii_case("changelog: none") {
        Some(MarkerKind::None)
    } else if t.eq_ignore_ascii_case("changelog") {
        Some(MarkerKind::Changelog)
    } else {
        None
    }
}

fn dequote(line: &str) -> String {
    let t = line.trim_start();
    let t = t.strip_prefix('>').unwrap_or(t);
    t.strip_prefix(' ').unwrap_or(t).to_owned()
}

fn parse_changelog_sections(lines: &[String]) -> Vec<ChangelogSection> {
    let mut sections: Vec<ChangelogSection> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("### ") {
            sections.push(ChangelogSection {
                name: name.trim().to_owned(),
                bullets: Vec::new(),
            });
        } else if let Some(bullet) = trimmed.strip_prefix("- ")
            && let Some(section) = sections.last_mut()
        {
            section.bullets.push(bullet.trim().to_owned());
        }
    }
    sections
}

/// Strips only `<!-- ... -->` HTML comments, keeping everything else verbatim.
fn strip_html_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find("-->") {
            rest = &rest[end + 3..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_well_formed_blockquote() {
        let body = "**Description**\n\nSome text.\n\n**Changelog**\n\n> ### Added\n>\n> - New thing.\n> - Another thing.\n";
        let extraction = extract_changelog(Some(body));
        assert_eq!(
            extraction,
            Extraction::Sections(vec![ChangelogSection {
                name: "Added".to_owned(),
                bullets: vec!["New thing.".to_owned(), "Another thing.".to_owned()],
            }])
        );
    }

    #[test]
    fn extraction_bold_optional_marker() {
        let body = "Changelog\n\n> ### Fixed\n>\n> - Fixed a bug.\n";
        let extraction = extract_changelog(Some(body));
        assert_eq!(
            extraction,
            Extraction::Sections(vec![ChangelogSection {
                name: "Fixed".to_owned(),
                bullets: vec!["Fixed a bug.".to_owned()],
            }])
        );
    }

    #[test]
    fn extraction_none_opt_out() {
        assert_eq!(extract_changelog(Some("**Changelog: None**")), Extraction::NoneOptOut);
        assert_eq!(extract_changelog(Some("Changelog: None")), Extraction::NoneOptOut);
    }

    #[test]
    fn extraction_missing_marker_is_placeholder() {
        assert_eq!(extract_changelog(Some("Just a description, no marker.")), Extraction::Placeholder);
        assert_eq!(extract_changelog(None), Extraction::Placeholder);
    }

    #[test]
    fn extraction_malformed_blockquote_is_placeholder() {
        // Marker present, but no blockquote follows.
        assert_eq!(extract_changelog(Some("**Changelog**\n\nNo blockquote here.")), Extraction::Placeholder);
        // Marker present, blockquote present, but no bullets.
        assert_eq!(
            extract_changelog(Some("**Changelog**\n\n> ### Added\n>\n")),
            Extraction::Placeholder
        );
    }

    #[test]
    fn extraction_strips_html_comments_only() {
        let body = "**Changelog**\n\n> ### Added <!-- Or Fixed, Changed, etc. -->\n>\n> - [<!-- Delete as appropriate. -->Breaking change:] Entry text.\n";
        let extraction = extract_changelog(Some(body));
        assert_eq!(
            extraction,
            Extraction::Sections(vec![ChangelogSection {
                name: "Added".to_owned(),
                bullets: vec!["[Breaking change:] Entry text.".to_owned()],
            }])
        );
    }
}
