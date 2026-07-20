// Copyright 2026 the Release Engineering Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Repository automation tasks - see <https://github.com/matklad/cargo-xtask>

use anyhow::Result;
use clap::{Parser, Subcommand};
use git_changelog::{Args, Config};

#[derive(Parser, Debug)]
#[command(bin_name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a first-draft changelog from merged pull requests.
    GenerateChangelog(Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::GenerateChangelog(args) => Config::new("DJMcNab/release_eng", "CHANGELOG.md")
            .changelog(
                "examples/test_package/CHANGELOG.md",
                ["examples/test_package"],
            )
            .run(args),
    }
}
