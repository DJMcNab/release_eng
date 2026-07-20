// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolving a squash-merge commit's `Co-authored-by:` trailers to GitHub logins.

use crate::collect::run_gh;
use anyhow::{Context as _, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

/// A `Co-authored-by:` trailer parsed from a commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CoAuthorTrailer {
    name: String,
    email: String,
}

/// Resolves the `Co-authored-by:` trailers on a squash commit to GitHub logins.
///
/// Trailers using a `users.noreply.github.com` address are mapped directly (no API call). If any
/// *other* trailers remain, a single `GET /pulls/{n}/commits` call provides an
/// `email -> login` map (built from the PR's real commit emails) to resolve them. Any trailer we
/// still cannot resolve produces a warning rather than being silently dropped.
pub(crate) fn resolve_pr_co_authors(repo: &str, number: u64, commit_message: &str) -> Vec<String> {
    let trailers = parse_co_author_trailers(commit_message);
    if trailers.is_empty() {
        return Vec::new();
    }

    let needs_lookup = trailers
        .iter()
        .any(|t| login_from_noreply(&t.email).is_none());
    let commit_login_map = if needs_lookup {
        match fetch_pr_commit_logins(repo, number) {
            Ok(map) => map,
            Err(err) => {
                eprintln!(
                    "warning: PR #{number}: failed to look up commit authors for co-author resolution: {err:#}"
                );
                BTreeMap::new()
            }
        }
    } else {
        BTreeMap::new()
    };

    let (resolved, unresolved) = resolve_co_authors(&trailers, &commit_login_map);
    for trailer in &unresolved {
        eprintln!(
            "warning: PR #{number}: could not resolve co-author `{} <{}>` to a GitHub login; add attribution manually",
            trailer.name, trailer.email
        );
    }
    resolved
}

/// Parses every `Co-authored-by:` trailer in a commit message into its name and email.
fn parse_co_author_trailers(commit_message: &str) -> Vec<CoAuthorTrailer> {
    commit_message
        .lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("Co-authored-by:")
                .or_else(|| line.strip_prefix("co-authored-by:"))?;
            parse_trailer_identity(rest.trim())
        })
        .collect()
}

/// Parses a `Name <email>` identity out of a trailer value.
fn parse_trailer_identity(value: &str) -> Option<CoAuthorTrailer> {
    let start = value.find('<')?;
    let end = value[start..].find('>')? + start;
    let email = value[start + 1..end].trim().to_owned();
    if email.is_empty() {
        return None;
    }
    Some(CoAuthorTrailer {
        name: value[..start].trim().to_owned(),
        email,
    })
}

/// Extracts the login from a `users.noreply.github.com` address (with or without the numeric-ID
/// prefix GitHub adds), or `None` for any other address.
fn login_from_noreply(email: &str) -> Option<String> {
    let (local, domain) = email.split_once('@')?;
    if !domain.eq_ignore_ascii_case("users.noreply.github.com") {
        return None;
    }
    let login = local.rsplit_once('+').map_or(local, |(_, l)| l);
    (!login.is_empty()).then(|| login.to_owned())
}

/// Resolves each trailer to a login, preferring the noreply address and falling back to the
/// PR-commit `email -> login` map. Returns the resolved logins (in trailer order) and the
/// trailers that could not be resolved.
fn resolve_co_authors(
    trailers: &[CoAuthorTrailer],
    commit_login_map: &BTreeMap<String, String>,
) -> (Vec<String>, Vec<CoAuthorTrailer>) {
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();
    for trailer in trailers {
        if let Some(login) = login_from_noreply(&trailer.email) {
            resolved.push(login);
        } else if let Some(login) = commit_login_map.get(&trailer.email.to_lowercase()) {
            resolved.push(login.clone());
        } else {
            unresolved.push(trailer.clone());
        }
    }
    (resolved, unresolved)
}

/// Fetches the PR's commits and builds a lowercased-`email -> login` map from them.
fn fetch_pr_commit_logins(repo: &str, number: u64) -> Result<BTreeMap<String, String>> {
    let endpoint = format!("/repos/{repo}/pulls/{number}/commits?per_page=100");
    let json = run_gh(&[
        "api",
        "-H",
        "Accept: application/vnd.github+json",
        &endpoint,
    ])
    .with_context(|| format!("failed to fetch commits for PR #{number}"))?;
    let commits: Vec<PrCommitResponse> = serde_json::from_str(&json)
        .with_context(|| format!("failed to parse commits JSON for PR #{number}"))?;
    Ok(build_email_login_map(&commits))
}

#[derive(Debug, Deserialize)]
struct PrCommitResponse {
    commit: PrCommitDetail,
    author: Option<PrCommitAccount>,
    committer: Option<PrCommitAccount>,
}

#[derive(Debug, Deserialize)]
struct PrCommitDetail {
    author: Option<PrGitIdentity>,
    committer: Option<PrGitIdentity>,
}

#[derive(Debug, Deserialize)]
struct PrGitIdentity {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrCommitAccount {
    login: String,
}

/// Builds a lowercased-`email -> login` map from a PR's commits. The git author's email maps to
/// the resolved author account; the committer's email fills in only where an author mapping is
/// absent.
fn build_email_login_map(commits: &[PrCommitResponse]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for commit in commits {
        if let Some(account) = &commit.author
            && let Some(identity) = &commit.commit.author
            && let Some(email) = &identity.email
        {
            map.insert(email.to_lowercase(), account.login.clone());
        }
    }
    for commit in commits {
        if let Some(account) = &commit.committer
            && let Some(identity) = &commit.commit.committer
            && let Some(email) = &identity.email
        {
            map.entry(email.to_lowercase())
                .or_insert_with(|| account.login.clone());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn co_author_trailers_parsed_and_resolved() {
        let message = "Do a thing (#42)\n\nBody.\n\n\
             Co-authored-by: Nores Ply <12345+noreply_user@users.noreply.github.com>\n\
             Co-authored-by: Jane Doe <Jane@Corp.Example>\n\
             Co-authored-by: Ghost <ghost@nowhere.example>\n";
        let trailers = parse_co_author_trailers(message);
        assert_eq!(trailers.len(), 3);
        assert_eq!(trailers[1].name, "Jane Doe");
        assert_eq!(trailers[1].email, "Jane@Corp.Example");

        // noreply resolves with no map; Jane resolves via the commit map (case-insensitive email);
        // Ghost cannot be resolved and is surfaced rather than dropped.
        let mut map = BTreeMap::new();
        map.insert("jane@corp.example".to_owned(), "janedoe".to_owned());
        let (resolved, unresolved) = resolve_co_authors(&trailers, &map);
        assert_eq!(
            resolved,
            vec!["noreply_user".to_owned(), "janedoe".to_owned()]
        );
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].email, "ghost@nowhere.example");
    }

    #[test]
    fn commit_login_map_built_from_pr_commits_json() {
        // Mirrors the shape of `GET /pulls/{n}/commits`: an unmatched author (`author: null`)
        // falls back to the committer login, and emails are lowercased.
        let json = r#"[
          {"commit":{"author":{"email":"Jane@Corp.Example"},"committer":{"email":"web@flow.example"}},
           "author":{"login":"janedoe"},"committer":{"login":"web-flow"}},
          {"commit":{"author":{"email":"x@y.example"},"committer":{"email":"x@y.example"}},
           "author":null,"committer":{"login":"fallbacklogin"}}
        ]"#;
        let commits: Vec<PrCommitResponse> = serde_json::from_str(json).expect("valid json");
        let map = build_email_login_map(&commits);
        assert_eq!(map.get("jane@corp.example"), Some(&"janedoe".to_owned()));
        assert_eq!(map.get("x@y.example"), Some(&"fallbacklogin".to_owned()));
    }
}
