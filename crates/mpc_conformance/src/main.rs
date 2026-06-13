use anyhow::Result;
use clap::Parser;
use mpc_conformance::run_fixture_path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mpc-conformance")]
#[command(about = "Runs MPC2000XL behavior fixtures against the deterministic core.")]
struct Args {
    #[arg(value_name = "FIXTURE_JSON")]
    fixture: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = run_fixture_path(args.fixture)?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    if report.passed {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
