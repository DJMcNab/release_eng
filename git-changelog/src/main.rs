//! CLI entrypoint for generating PR-body reports from commit ranges.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Fetch PR bodies for each commit in a local commit range and write them to JSON")]
struct Args {
    /// Start commit/range bound (exclusive)
    from_commit: String,
    /// End commit/range bound (inclusive)
    to_commit: String,
    /// Output JSON path. Defaults to pr_bodies_<from>_<to>.json
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = git_changelog::fetch_pr_report(&args.from_commit, &args.to_commit)?;
    let output_path = args
        .output
        .unwrap_or_else(|| git_changelog::default_output_path(&args.from_commit, &args.to_commit));
    git_changelog::write_report_json(&report, &output_path)?;

    println!(
        "Wrote PR body report for {} commits to {}",
        report.commits.len(),
        output_path.display()
    );
    Ok(())
}
