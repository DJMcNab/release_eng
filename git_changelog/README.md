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
cargo rdme --workspace-project=git-changelog --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs should be evaluated here.
See https://linebender.org/blog/doc-include/ for related discussion. -->
<!-- cargo-rdme start -->

A reusable first-draft changelog generator.

Linebender projects keep hand-curated, keep-a-changelog-style `CHANGELOG.md` files: a
`## [Unreleased]` section, `### Added`/`### Changed`/... subsections, entries suffixed
`([#1234][] by [@author][])`, and reference-style link definitions collected at the bottom
of the file.

This crate finds the pull requests merged since the last run, extracts their changelog
sections from their PR bodies, routes each PR to the right changelog file(s) based on which
paths it touched, and merges the resulting entries into `## [Unreleased]`, sorted by PR
number. The output is a first draft: a human is expected to curate it afterwards.

The pipeline has three stages:

1. `collect` (impure: talks to `git` and `gh`) gathers everything needed from the outside
   world into a `Collected` value.
2. `process` (pure: no I/O at all) takes a `Collected` plus the current contents of the
   target changelog files and computes the new file contents.
3. `apply` (impure) does a pre-flight dirty check, reads the current file contents, calls
   `process`, writes the results, and advances the marker last.

A calling `xtask` looks like:

```rust
use clap::Parser as _;
git_changelog::Config::new("linebender/vello", "CHANGELOG.md")
    .changelog("sparse_strips/vello_cpu/CHANGELOG.md", ["sparse_strips/vello_cpu"])
    .changelog("sparse_strips/vello_common/CHANGELOG.md", ["sparse_strips/vello_common"])
    .run(git_changelog::Args::parse())?;
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of git-changelog has been verified to compile with **Rust 1.96** and later.

Future versions of git-changelog might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

<details>
<summary>Click here if compiling fails.</summary>

As time has passed, some of git-changelog's dependencies could have released versions with a higher Rust requirement.
If you encounter a compilation issue due to a dependency and don't want to upgrade your Rust toolchain, then you could downgrade the dependency.

```sh
# Use the problematic dependency's name and version
cargo update -p package_name --precise 0.1.1
```

</details>

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
