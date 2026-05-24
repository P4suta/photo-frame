//! Command-line driver for `photo-frame-core`. Reads files, calls
//! [`photo_frame_core::frame_image`], writes files, surfaces every failure
//! and unusual event through `tracing` to stderr.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser};
use photo_frame_core::{
    frame_image, Background, ErrorCategory, FrameError, FrameOptions, MetaPolicy,
};
use tracing::{error, info, instrument, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Liit-style golden-ratio photo framing with embedded EXIF caption.
#[derive(Debug, Parser)]
#[command(name = "photo-frame", version, about, long_about = None)]
struct Cli {
    /// Input image(s) (JPEG or PNG). May be repeated to batch-process.
    #[arg(required = true, value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    /// Output path. Defaults to `<input_stem>_framed.jpg` alongside each
    /// input. When more than one input is given, this is treated as a
    /// directory.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// JPEG quality, 1..=100.
    #[arg(short, long, default_value_t = 92, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: u8,

    /// Suppress the metadata strip even if EXIF is present.
    #[arg(long)]
    no_meta: bool,

    /// Downscale so the longer edge is at most this many pixels.
    #[arg(long, value_name = "PX")]
    max_long_edge: Option<u32>,

    /// Frame background color as `#RRGGBB` or `RRGGBB`.
    #[arg(short, long, value_name = "HEX", default_value = "#FFFFFF", value_parser = parse_hex_color)]
    background: Background,

    /// Increase log verbosity (`-v`=debug, `-vv`=trace).
    #[arg(short, long, action = ArgAction::Count, conflicts_with = "quiet")]
    verbose: u8,

    /// Only emit warnings and errors.
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Emit one JSON event per line to stderr instead of pretty text.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet, cli.json);

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report_error(&err);
            // If the error chain bottoms out at a FrameError, honour its
            // category code; otherwise generic failure.
            ExitCode::from(u8::try_from(exit_code_for(&err)).unwrap_or(1))
        },
    }
}

#[instrument(skip_all, fields(inputs = cli.inputs.len()))]
fn run(cli: &Cli) -> Result<()> {
    let opts = FrameOptions {
        jpeg_quality: cli.quality,
        background: cli.background,
        meta_policy: if cli.no_meta {
            MetaPolicy::Never
        } else {
            MetaPolicy::Auto
        },
        max_long_edge: cli.max_long_edge,
    };

    let single = cli.inputs.len() == 1;
    for input in &cli.inputs {
        let output = resolve_output(input, cli.output.as_deref(), single);
        process_one(input, &output, &opts)
            .with_context(|| format!("processing {}", input.display()))?;
        info!(input = %input.display(), output = %output.display(), "wrote framed output");
        println!("{} → {}", input.display(), output.display());
    }
    Ok(())
}

#[instrument(skip(opts))]
fn process_one(input: &Path, output: &Path, opts: &FrameOptions) -> Result<()> {
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    let framed = frame_image(&bytes, opts).context("framing")?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    std::fs::write(output, framed).with_context(|| format!("writing {}", output.display()))?;
    Ok(())
}

fn init_tracing(verbose: u8, quiet: bool, json: bool) {
    let default = if quiet {
        Level::WARN
    } else {
        match verbose {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    };
    let filter = EnvFilter::try_from_env("PHOTO_FRAME_LOG")
        .unwrap_or_else(|_| EnvFilter::new(default.to_string()));
    let registry = tracing_subscriber::registry().with(filter);
    if json {
        registry
            .with(
                fmt::layer()
                    .json()
                    .with_writer(std::io::stderr)
                    .with_target(false),
            )
            .init();
    } else {
        registry
            .with(
                fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(false)
                    .with_timer(fmt::time::uptime()),
            )
            .init();
    }
}

/// Print the full cause chain so operators can see every layer of context.
fn report_error(err: &anyhow::Error) {
    error!(error = %err, "command failed");
    eprintln!("error: {err}");
    for (i, cause) in err.chain().skip(1).enumerate() {
        eprintln!("  caused by [{i}]: {cause}");
    }
}

fn exit_code_for(err: &anyhow::Error) -> i32 {
    err.chain()
        .find_map(|e| e.downcast_ref::<FrameError>())
        .map_or(1, |fe| ErrorCategory::exit_code(fe.category()))
}

fn resolve_output(input: &Path, output: Option<&Path>, single: bool) -> PathBuf {
    match output {
        None => default_output_path(input),
        Some(path) if single && !is_existing_dir(path) => path.to_owned(),
        Some(dir) => dir.join(
            default_output_path(input)
                .file_name()
                .expect("default_output_path always provides a file name"),
        ),
    }
}

fn default_output_path(input: &Path) -> PathBuf {
    let stem = input.file_stem().unwrap_or(input.as_os_str());
    let mut name = stem.to_owned();
    name.push("_framed.jpg");
    input.with_file_name(name)
}

fn is_existing_dir(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_dir())
}

fn parse_hex_color(s: &str) -> Result<Background> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        bail!("color must be 6 hex digits (e.g. #FFFFFF); got `{s}`");
    }
    let r = u8::from_str_radix(&hex[0..2], 16).context("red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).context("green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).context("blue component")?;
    Ok(Background::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::{exit_code_for, parse_hex_color};
    use photo_frame_core::{Background, FrameError};

    #[test]
    fn parses_hex_with_hash() {
        assert_eq!(
            parse_hex_color("#ff8040").unwrap(),
            Background::from_rgb(255, 128, 64)
        );
    }

    #[test]
    fn parses_hex_without_hash() {
        assert_eq!(parse_hex_color("FFFFFF").unwrap(), Background::WHITE);
    }

    #[test]
    fn rejects_short_input() {
        assert!(parse_hex_color("#FFF").is_err());
    }

    #[test]
    fn rejects_non_hex() {
        assert!(parse_hex_color("#GGGGGG").is_err());
    }

    #[test]
    fn exit_code_walks_chain_for_frame_error() {
        let err: anyhow::Error =
            anyhow::Error::new(FrameError::EmptyInput).context("processing photo.jpg");
        assert_eq!(exit_code_for(&err), 2);
    }

    #[test]
    fn exit_code_defaults_to_one_for_unknown_error() {
        let err = anyhow::anyhow!("totally unrelated");
        assert_eq!(exit_code_for(&err), 1);
    }
}
