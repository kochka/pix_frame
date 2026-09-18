//! `pixframe`: desktop tool for managing a pix-frame device.
//!
//! Today it builds and inspects the device configuration image written to the
//! `nvs` partition (see `partitions.csv` and `config.example.toml`).

mod config;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use config::Config;

/// Size of the `nvs` partition in `partitions.csv`.
const DEFAULT_NVS_SIZE: usize = 0x6000;

#[derive(Parser)]
#[command(name = "pixframe", version, about = "pix-frame device management")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build or inspect the device configuration image.
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Build the NVS image from a TOML configuration file.
    Build(BuildArgs),
    /// Print the configuration stored in an NVS image.
    Dump { image: PathBuf },
}

#[derive(Args)]
struct BuildArgs {
    /// Configuration file, see `config.example.toml`.
    config: PathBuf,
    /// Image to write, e.g. `nvs.bin`.
    #[arg(short, long)]
    output: PathBuf,
    /// Partition size in bytes; must match the `nvs` entry of `partitions.csv`.
    #[arg(long, default_value_t = DEFAULT_NVS_SIZE, value_parser = parse_size)]
    size: usize,
}

fn parse_size(text: &str) -> Result<usize> {
    let text = text.trim();
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => usize::from_str_radix(hex, 16),
        None => text.parse(),
    };
    value.with_context(|| format!("invalid size {text:?}"))
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Config(ConfigCommand::Build(args)) => {
            let text = std::fs::read_to_string(&args.config)
                .with_context(|| format!("cannot read {}", args.config.display()))?;
            let config =
                Config::parse(&text).with_context(|| format!("{}", args.config.display()))?;
            let image = config.to_nvs_image(args.size)?;
            std::fs::write(&args.output, &image)
                .with_context(|| format!("cannot write {}", args.output.display()))?;
            print!("{}", config.summary());
            println!("{}: {} bytes", args.output.display(), image.len());
            Ok(())
        }
        Command::Config(ConfigCommand::Dump { image }) => {
            let bytes = std::fs::read(&image)
                .with_context(|| format!("cannot read {}", image.display()))?;
            let config =
                Config::from_nvs_image(bytes).with_context(|| format!("{}", image.display()))?;
            print!("{}", config.summary());
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
