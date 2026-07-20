<div align="center">

# Release Engineering

**Experiments in release engineering for Linebender projects**

[![Latest published version.](https://img.shields.io/crates/v/release_eng.svg)](https://crates.io/crates/release_eng)
[![Documentation build status.](https://img.shields.io/docsrs/release_eng.svg)](https://docs.rs/release_eng)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![Linebender Zulip chat.](https://img.shields.io/badge/Linebender-%23general-blue?logo=Zulip)](https://xi.zulipchat.com/#narrow/channel/147921-general)
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/DJMcNab/release_eng/ci.yml?logo=github&label=CI)](https://github.com/DJMcNab/release_eng/actions)
[![Dependency staleness status.](https://deps.rs/crate/release_eng/latest/status.svg)](https://deps.rs/crate/release_eng)

</div>

This repository collects tooling used to help release [Linebender](https://linebender.org/) projects.
It is a work in progress and the tools here are experimental.

## Crates

- [`git-changelog`](./git-changelog): A reusable library which drafts a first-pass changelog by
  reading the `Changelog` blocks of merged pull requests and merging them into the `## [Unreleased]`
  section of a [keep-a-changelog]-style `CHANGELOG.md`.
- [`changelog-to-release`](./changelog-to-release): Extracts the notes for a specific release from a
  `CHANGELOG.md`.
- [`xtask`](./xtask): Repository automation. Run `cargo xtask --since <git-ref>` to draft changelog
  entries using `git-changelog`.

The `examples/test-package` crate exists only to exercise the tooling and is not published.

## Minimum supported Rust Version (MSRV)

This version of Release Engineering has been verified to compile with **Rust 1.96** and later.

Future versions of Release Engineering might increase the Rust version requirement.
It will not be treated as a breaking change and as such can even happen with small patch releases.

## Community

Discussion of Release Engineering development happens in the [Linebender Zulip](https://xi.zulipchat.com/), specifically the [#general channel](https://xi.zulipchat.com/#narrow/channel/147921-general).
All public content can be read without logging in.

Contributions are welcome by pull request. The [Rust code of conduct] applies.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache 2.0 license, shall be licensed as noted in the [License](#license) section, without any additional terms or conditions.

When contributing (both issues and pull requests), you must disclose LLM-generated content ahead of time.
See [LLM contribution policy for Linebender projects](https://linebender.org/wiki/llm-policy/) for details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[keep-a-changelog]: https://keepachangelog.com/en/1.1.0/
