//! Command-line driver for the `photo-frame` facade.
//!
//! Reads files, calls [`photo_frame::pipeline`], writes the framed JPEG
//! to disk. Every failure path surfaces through `anyhow`'s cause chain;
//! observability lives in `tracing` events sent to stderr.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use photo_frame::frame::{Background, MetaPolicy};
use photo_frame::{
    pipeline, DecodeError, EncodeError, PipelineError, PipelineOptions, QualityPreset,
};
use tracing::{error, info, instrument, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Liit-style golden-ratio photo framing with embedded EXIF caption.
#[derive(Debug, Parser)]
#[command(name = "photo-frame", version, about, long_about = None)]
struct Cli {
    /// Input image(s) (JPEG / PNG / TIFF / BMP / WebP; HEIC under the
    /// `heif` feature). May be repeated to batch-process.
    #[arg(required = true, value_name = "INPUT")]
    inputs: Vec<PathBuf>,

    /// Output path. Defaults to `<input_stem>_framed.jpg` alongside each
    /// input. When more than one input is given, this is treated as a
    /// directory.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Quality / size bundle. Individual --quality and --max-long-edge
    /// flags override the preset's values.
    #[arg(long, value_enum, default_value_t = CliPreset::Standard)]
    preset: CliPreset,

    /// Override the preset's JPEG quality (1..=100).
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(1..=100))]
    quality: Option<u8>,

    /// Override the preset's longer-edge pixel cap. Omit to inherit the
    /// preset value (SNS = 2048, others = no cap).
    #[arg(long, value_name = "PX")]
    max_long_edge: Option<u32>,

    /// Suppress the metadata strip even if EXIF is present.
    #[arg(long)]
    no_meta: bool,

    /// Frame background colour as `#RRGGBB` or `RRGGBB`.
    #[arg(short, long, value_name = "HEX", default_value = "#FFFFFF", value_parser = parse_hex_color)]
    background: Background,

    /// Increase log verbosity (`-v`=debug, `-vv`=trace).
    #[arg(short, long, action = ArgAction::Count, conflicts_with = "quiet")]
    verbose: u8,

    /// Only emit warnings and errors.
    #[arg(long, conflicts_with = "verbose")]
    quiet: bool,

    /// Emit one JSON event per line to stderr instead of pretty text.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum CliPreset {
    /// Optimised for SNS upload: small file, long edge ≤ 2048 px.
    Sns,
    /// Balanced default. No downscale.
    Standard,
    /// Highest quality, no downscale.
    Maximum,
}

impl From<CliPreset> for QualityPreset {
    fn from(value: CliPreset) -> Self {
        match value {
            CliPreset::Sns => Self::Sns,
            CliPreset::Standard => Self::Standard,
            CliPreset::Maximum => Self::Maximum,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet, cli.json);

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report_error(&err);
            ExitCode::from(exit_code_for(&err))
        },
    }
}

#[instrument(skip_all, fields(inputs = cli.inputs.len(), preset = ?cli.preset))]
fn run(cli: &Cli) -> Result<()> {
    let opts = build_options(cli);

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

/// Materialise [`PipelineOptions`] from the parsed CLI. Preset supplies
/// the baseline; individual flags layer on top of it.
fn build_options(cli: &Cli) -> PipelineOptions {
    let preset: QualityPreset = cli.preset.into();
    let mut opts = PipelineOptions::from_preset(preset);
    if let Some(q) = cli.quality {
        opts.jpeg.quality = q;
    }
    // `cli.max_long_edge` being `Some(_)` is the *user-explicitly-asked*
    // signal; `None` means "inherit from preset", which we already did.
    if cli.max_long_edge.is_some() {
        opts.frame.max_long_edge = cli.max_long_edge;
    }
    opts.frame.background = cli.background;
    opts.frame.meta_policy = if cli.no_meta {
        MetaPolicy::Never
    } else {
        MetaPolicy::Auto
    };
    opts
}

#[instrument(skip(opts))]
fn process_one(input: &Path, output: &Path, opts: &PipelineOptions) -> Result<()> {
    let bytes = std::fs::read(input).with_context(|| format!("reading {}", input.display()))?;
    let framed = pipeline(&bytes, opts).context("framing")?;
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

/// Print the full cause chain so operators see every layer of context.
fn report_error(err: &anyhow::Error) {
    error!(error = %err, "command failed");
    eprintln!("error: {err}");
    for (i, cause) in err.chain().skip(1).enumerate() {
        eprintln!("  caused by [{i}]: {cause}");
    }
}

/// Map a `PipelineError` (somewhere in the cause chain) to a stable exit
/// code so shell scripts can react. The mapping deliberately mirrors
/// v1.3's `ErrorCategory`: 2 for input problems (empty bytes / unknown
/// format), 3 for decode failures, 4 for encode failures, 1 otherwise.
fn exit_code_for(err: &anyhow::Error) -> u8 {
    err.chain()
        .find_map(|e| e.downcast_ref::<PipelineError>())
        .map_or(1, classify)
}

const fn classify(err: &PipelineError) -> u8 {
    match err {
        PipelineError::Decode(decode) => match decode {
            DecodeError::EmptyInput | DecodeError::UnknownFormat => 2,
            _ => 3,
        },
        PipelineError::Encode(EncodeError::InvalidQuality { .. }) => 2,
        PipelineError::Encode(_) => 4,
    }
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
        bail!("colour must be 6 hex digits (e.g. #FFFFFF); got `{s}`");
    }
    let r = u8::from_str_radix(&hex[0..2], 16).context("red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).context("green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).context("blue component")?;
    Ok(Background::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::{build_options, classify, exit_code_for, parse_hex_color, Cli, CliPreset};
    use clap::Parser;
    use photo_frame::frame::Background;
    use photo_frame::{DecodeError, PipelineError, QualityPreset};

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
    fn exit_code_walks_chain_for_pipeline_error() {
        let err: anyhow::Error = anyhow::Error::new(PipelineError::Decode(DecodeError::EmptyInput))
            .context("processing photo.jpg");
        assert_eq!(exit_code_for(&err), 2);
    }

    #[test]
    fn exit_code_defaults_to_one_for_unknown_error() {
        let err = anyhow::anyhow!("totally unrelated");
        assert_eq!(exit_code_for(&err), 1);
    }

    #[test]
    fn classify_maps_decode_subvariants() {
        assert_eq!(classify(&PipelineError::Decode(DecodeError::EmptyInput)), 2);
        assert_eq!(
            classify(&PipelineError::Decode(DecodeError::UnknownFormat)),
            2
        );
        assert_eq!(
            classify(&PipelineError::Decode(DecodeError::HeifFeatureDisabled)),
            3
        );
    }

    #[test]
    fn cli_preset_maps_to_facade_preset() {
        assert!(matches!(
            QualityPreset::from(CliPreset::Sns),
            QualityPreset::Sns
        ));
        assert!(matches!(
            QualityPreset::from(CliPreset::Standard),
            QualityPreset::Standard
        ));
        assert!(matches!(
            QualityPreset::from(CliPreset::Maximum),
            QualityPreset::Maximum
        ));
    }

    #[test]
    fn default_preset_drives_standard_options() {
        let cli = Cli::try_parse_from(["photo-frame", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.jpeg.quality, 92);
        assert_eq!(opts.frame.max_long_edge, None);
    }

    #[test]
    fn sns_preset_caps_long_edge() {
        let cli = Cli::try_parse_from(["photo-frame", "--preset", "sns", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.jpeg.quality, 78);
        assert_eq!(opts.frame.max_long_edge, Some(2048));
    }

    #[test]
    fn maximum_preset_bumps_quality() {
        let cli = Cli::try_parse_from(["photo-frame", "--preset", "maximum", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.jpeg.quality, 98);
        assert_eq!(opts.frame.max_long_edge, None);
    }

    #[test]
    fn explicit_quality_overrides_preset() {
        let cli = Cli::try_parse_from(["photo-frame", "--preset", "sns", "-q", "95", "photo.jpg"])
            .unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.jpeg.quality, 95);
        // Preset's long-edge cap survives — only the quality was overridden.
        assert_eq!(opts.frame.max_long_edge, Some(2048));
    }

    #[test]
    fn explicit_max_long_edge_overrides_preset() {
        let cli = Cli::try_parse_from([
            "photo-frame",
            "--preset",
            "maximum",
            "--max-long-edge",
            "3000",
            "photo.jpg",
        ])
        .unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.frame.max_long_edge, Some(3000));
        assert_eq!(opts.jpeg.quality, 98);
    }
}
