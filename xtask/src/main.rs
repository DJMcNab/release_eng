//! Repository automation tasks for `release_eng`.
//!
//! Currently the only task drives changelog generation via the [`git_changelog`] library. Run it
//! with:
//!
//! ```sh
//! cargo xtask generate-changelog
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use git_changelog::{Args, Config};

/// Repository automation tasks for `release_eng`.
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
                "examples/test-package/CHANGELOG.md",
                ["examples/test-package"],
            )
            .run(args),
    }
}
