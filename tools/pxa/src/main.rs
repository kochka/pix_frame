//! `pxa`: desktop tool that converts animations to the PXA format and
//! inspects PXA files. See `doc/PXA_spec.md` section 17.

mod animation;
mod convert;
mod import;
mod inspect;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use pxa_core::Rgb;

#[derive(Parser)]
#[command(name = "pxa", version, about = "PXA animation converter and inspector")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert one or more GIF, PNG, APNG or WebP animations to PXA.
    Convert(ConvertArgs),
    /// Print the header, palette and frame list of a PXA file.
    Inspect { file: PathBuf },
    /// Check that a file is a conforming PXA v1 file.
    Validate { file: PathBuf },
}

#[derive(Args)]
struct ConvertArgs {
    /// Source animations (GIF, PNG, APNG, WebP). Several inputs are converted in turn.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output file for a single input, or an existing directory.
    /// Defaults to the input name with a `.pxa` extension, next to the input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Resize to WxH with nearest-neighbour sampling, e.g. `16x16`.
    #[arg(long, value_parser = parse_size)]
    size: Option<(u16, u16)>,
    /// Background color composited under transparent pixels, as RRGGBB.
    #[arg(long, default_value = "000000", value_parser = parse_color)]
    background: Rgb,
    /// Duration in ms given to source frames whose delay is zero.
    /// Without it, zero-delay frames are omitted. Also used for static images.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    default_duration: Option<u16>,
    /// Maximum palette size. Sources with more colors are quantized.
    #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u16).range(1..=256))]
    colors: u16,
}

fn parse_size(text: &str) -> Result<(u16, u16)> {
    let (w, h) = text
        .split_once(['x', 'X', '×'])
        .context("expected WIDTHxHEIGHT, e.g. 16x16")?;
    let width: u16 = w.trim().parse().context("invalid width")?;
    let height: u16 = h.trim().parse().context("invalid height")?;
    if width == 0 || height == 0 {
        bail!("width and height must be non-zero");
    }
    Ok((width, height))
}

fn parse_color(text: &str) -> Result<Rgb> {
    let hex = text.trim_start_matches('#');
    if hex.len() != 6 {
        bail!("expected six hex digits, e.g. 000000 or #FF8800");
    }
    let value = u32::from_str_radix(hex, 16).context("invalid hex color")?;
    Ok([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Convert(args) => convert_all(&args),
        Command::Inspect { file } => {
            let bytes =
                std::fs::read(&file).with_context(|| format!("cannot read {}", file.display()))?;
            inspect::inspect(&file, &bytes)
        }
        Command::Validate { file } => {
            let bytes =
                std::fs::read(&file).with_context(|| format!("cannot read {}", file.display()))?;
            let parsed = pxa_core::PxaFile::parse(&bytes)
                .map_err(|e| anyhow::anyhow!("{}: invalid PXA file: {e}", file.display()))?;
            let header = parsed.header();
            println!(
                "{}: valid PXA v1, {}x{}, {} frames, {} colors",
                file.display(),
                header.width(),
                header.height(),
                header.frame_count(),
                header.palette_size()
            );
            Ok(())
        }
    }
}

/// Pairs every input with its output path.
fn resolve_outputs(inputs: &[PathBuf], output: Option<&Path>) -> Result<Vec<(PathBuf, PathBuf)>> {
    let pairs: Vec<(PathBuf, PathBuf)> = match output {
        None => inputs
            .iter()
            .map(|input| (input.clone(), input.with_extension("pxa")))
            .collect(),
        Some(output) if inputs.len() == 1 && !output.is_dir() => {
            vec![(inputs[0].clone(), output.to_path_buf())]
        }
        Some(directory) => {
            if !directory.is_dir() {
                bail!(
                    "{} is not an existing directory; --output must be a directory when converting several files",
                    directory.display()
                );
            }
            inputs
                .iter()
                .map(|input| {
                    let name = input
                        .file_name()
                        .with_context(|| format!("{} has no file name", input.display()))?;
                    Ok((input.clone(), directory.join(name).with_extension("pxa")))
                })
                .collect::<Result<_>>()?
        }
    };

    let mut seen: HashMap<&Path, &Path> = HashMap::new();
    for (input, output) in &pairs {
        if input == output {
            bail!("{} would overwrite its own input", output.display());
        }
        if let Some(previous) = seen.insert(output, input) {
            bail!(
                "{} and {} would both be written to {}",
                previous.display(),
                input.display(),
                output.display()
            );
        }
    }
    Ok(pairs)
}

fn convert_one(input: &Path, output: &Path, options: &convert::Options) -> Result<()> {
    let animation =
        import::load(input).with_context(|| format!("cannot load {}", input.display()))?;
    let report = convert::convert(&animation, options)?;
    std::fs::write(output, &report.bytes)
        .with_context(|| format!("cannot write {}", output.display()))?;
    for note in &report.notes {
        eprintln!("note: {}: {note}", input.display());
    }
    println!(
        "{} -> {}: {}x{}, {} frames, {} colors ({} bpp), {} bytes",
        input.display(),
        output.display(),
        report.width,
        report.height,
        report.frame_count,
        report.palette_size,
        report.bits_per_pixel,
        report.bytes.len()
    );
    Ok(())
}

/// Converts every input; in bulk mode a failure does not stop the others.
fn convert_all(args: &ConvertArgs) -> Result<()> {
    let options = convert::Options {
        size: args.size,
        background: args.background,
        default_duration: args.default_duration,
        max_colors: args.colors,
    };
    let pairs = resolve_outputs(&args.inputs, args.output.as_deref())?;

    if let [(input, output)] = pairs.as_slice() {
        return convert_one(input, output, &options);
    }

    let mut failures = 0usize;
    for (input, output) in &pairs {
        if let Err(error) = convert_one(input, output, &options) {
            eprintln!("error: {}: {error:#}", input.display());
            failures += 1;
        }
    }
    if failures > 0 {
        bail!("{failures} of {} conversions failed", pairs.len());
    }
    Ok(())
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
