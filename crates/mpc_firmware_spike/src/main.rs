use anyhow::Result;
use clap::Parser;
use mpc_firmware_spike::inspect_image;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mpc-firmware-spike")]
#[command(about = "Inspects user-supplied MPC2000XL OS images without storing firmware bytes.")]
struct Args {
    #[arg(value_name = "OS_IMAGE")]
    image: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = inspect_image(args.image)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
