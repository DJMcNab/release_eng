<div align="center">

# git-changelog

**A reusable first-draft changelog generator for Linebender projects**

[![Latest published version.](https://img.shields.io/crates/v/git-changelog.svg)](https://crates.io/crates/git-changelog)
[![Documentation build status.](https://img.shields.io/docsrs/git-changelog.svg)](https://docs.rs/git-changelog)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23general-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/147921-general)
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/DJMcNab/release_eng/ci.yml?logo=github&label=CI)](https://github.com/DJMcNab/release_eng/actions)
[![Dependency staleness status.](https://deps.rs/crate/git-changelog/latest/status.svg)](https://deps.rs/crate/git-changelog)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=git_changelog
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->

[`Config`]: https://docs.rs/git_changelog/latest/git_changelog/struct.Config.html

<!-- cargo-rdme start -->

First-draft changelog generation for single-or-multi crate projects, for use as an `xtask`.

This project currently requires that the repository is hosted on GitHub, and uses squash merges.
This is the process used by Linebender.
It supports routing changelog entries to all impacted crates for crates in a workspace which
maintain their own changelogs.

## Usage

In a repository which has this setup as an xtask, run it as:

```sh
cargo xtask generate-changelog
```

After ensuring that you're on the primary branch of the repository.
Once this command finishes, each CHANGELOG in the repository will have unstaged changes.
Use these as a starting point to create the new release's changelog.

The entries are collected from a quoted section in each PR which follows a **Changelog** marker.
To explicitly indicate that a PR does not require a changelog entry, replace this with **Changelog: None**.

The entries will be merged into the 'Unreleased' section of the relevant CHANGELOG, inferred from the
files changed in the PR.
You must then manually review these entries, and edit the CHANGELOG based on them.

## Motivation

A traditional workflow for as-you-go changelog generation is for all PRs with relevant
changes to also edit the CHANGELOG.md file to add their entry.
However, as we've used this in Linebender, we ran into several issues:

- It isn't clear if a changelog has been forgotten, or if the author intentionally decided it wasn't needed.
- It's very easy for edits in the CHANGELOG file to generate conflicts.
- It's possible for CHANGELOG entries to accidentally end up in the wrong place, if a
  release happens between a PR being opened and merged.

Systems which track in-progress changelogs using in-tree files avoid conflicts, but have issues
with approachability for users.
They also require choosing where to store the data.
This approach avoids this by storing the data in a PR description, which a contributor will already need to fill out.
Additionally, this gives maintainers a low-friction way to edit the changelog entry for a PR, either before or after merge.

## Setup

See the docs on [`Config`] for detailed setup instructions.

## Inspirations

The workflow in this crate is inspired by the conventions used in the Clippy repository (<https://github.com/rust-lang/rust-clippy>).
We however automate the process slightly more than is done in Clippy, to make updating changelogs require less manual work.

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Release Engineering has been verified to compile with **Rust 1.96** and later.

Future versions of Release Engineering might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

## Community

Discussion of Release Engineering development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#general channel](https://xi.zulipchat.com/#narrow/channel/147921-general).
All public content can be read without logging in.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.

[Rust Code of Conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: ../AUTHORS
