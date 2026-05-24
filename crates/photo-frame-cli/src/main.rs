//! Command-line driver for the `photo-frame` facade.
//!
//! Reads files, calls [`photo_frame::pipeline`], writes the framed
//! JPEG to disk. Every failure surfaces through `miette::Result`, so a
//! rich Unicode-boxed diagnostic with code + help text lands on stderr
//! whenever something goes wrong. Observability lives in `tracing`
//! events also sent to stderr.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{ArgAction, Parser, ValueEnum};
use miette::{Diagnostic, Result, WrapErr};
use photo_frame::frame::{Background, MetaPolicy};
use photo_frame::{pipeline, Categorize, Category, PipelineError, PipelineOptions, QualityPreset};
use thiserror::Error;
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
    /// Deprecated: prefer `--log-format json`. Honoured as a shortcut
    /// for `--log-format json` for backward compatibility.
    #[arg(long)]
    json: bool,

    /// Tracing-event output format on stderr.
    ///
    /// - `pretty`: default; multi-line, ANSI-coloured, human-friendly
    /// - `compact`: single-line per event, no ANSI; good for terminals
    ///   that don't render colour
    /// - `json`: one JSON object per line; structured fields preserved;
    ///   CI / log-aggregator friendly
    #[arg(long, value_enum)]
    log_format: Option<LogFormat>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum LogFormat {
    Pretty,
    Compact,
    Json,
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

/// CLI-side file I/O errors. The pipeline crate doesn't see the
/// filesystem, so a missing input / unwritable output surfaces here.
/// Categorising as Input keeps the exit code shell-script friendly.
#[derive(Debug, Error, Diagnostic)]
enum CliIoError {
    #[error("could not read input file: {path}")]
    #[diagnostic(
        code(photo_frame::cli::io::read),
        help("Check the path exists and is readable by the current user.")
    )]
    ReadInput {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write output file: {path}")]
    #[diagnostic(
        code(photo_frame::cli::io::write),
        help("Check the parent directory exists and is writable.")
    )]
    WriteOutput {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("could not create output directory: {path}")]
    #[diagnostic(code(photo_frame::cli::io::mkdir))]
    MakeDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl Categorize for CliIoError {
    fn category(&self) -> Category {
        Category::Input
    }
}

/// Hex-colour parse error used as a clap `value_parser` result. miette
/// renders it identically to the workspace pipeline errors.
#[derive(Debug, Error, Diagnostic)]
enum HexColorError {
    #[error("colour must be 6 hex digits (e.g. #FFFFFF); got `{got}`")]
    #[diagnostic(
        code(photo_frame::cli::hex_color::length),
        help("Strip whitespace and use the `#RRGGBB` form, e.g. #FFFFFF or 1A2B3C.")
    )]
    BadLength { got: String },
    #[error("colour component `{component}` is not valid hex: {source}")]
    #[diagnostic(
        code(photo_frame::cli::hex_color::digits),
        help("Each pair of hex digits must be 0-9 / a-f / A-F.")
    )]
    BadDigit {
        component: &'static str,
        #[source]
        source: std::num::ParseIntError,
    },
}

fn main() -> ExitCode {
    install_panic_hook();
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet, resolve_log_format(&cli));

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Render the diagnostic to stderr via miette's fancy handler
            // (already installed by the `fancy` feature flag at crate
            // boot). Falling back to {err:?} retains the same rendering.
            eprintln!("{err:?}");
            // Walk the cause chain looking for the first Categorize-
            // capable error to derive the exit code.
            ExitCode::from(exit_code_for(err.as_ref()))
        },
    }
}

/// Install a panic hook that emits a structured tracing event before
/// abort, so panics are visible to whoever's tailing logs (CI, journald,
/// kubectl logs etc.) and not just dumped to stderr.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(|| "<unknown>".to_string(), ToString::to_string);
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&'static str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no message)");
        error!(
            location = %location,
            panic.message = %message,
            "command panicked"
        );
        default_hook(info);
    }));
}

#[instrument(skip_all, fields(inputs = cli.inputs.len(), preset = ?cli.preset))]
fn run(cli: &Cli) -> Result<()> {
    let opts = build_options(cli);

    let single = cli.inputs.len() == 1;
    for input in &cli.inputs {
        let output = resolve_output(input, cli.output.as_deref(), single);
        process_one(input, &output, &opts)
            .wrap_err_with(|| format!("processing {}", input.display()))?;
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

#[instrument(skip(opts), fields(input = %input.display(), output = %output.display()))]
fn process_one(input: &Path, output: &Path, opts: &PipelineOptions) -> Result<()> {
    let bytes = std::fs::read(input).map_err(|source| CliIoError::ReadInput {
        path: input.display().to_string(),
        source,
    })?;
    let framed = pipeline(&bytes, opts).wrap_err("framing")?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| CliIoError::MakeDir {
                path: parent.display().to_string(),
                source,
            })?;
        }
    }
    std::fs::write(output, framed).map_err(|source| CliIoError::WriteOutput {
        path: output.display().to_string(),
        source,
    })?;
    Ok(())
}

/// Decide which log format the subscriber should produce.
/// `--log-format` wins outright; `--json` is honoured as a back-compat
/// shortcut for `json`; otherwise default to `pretty`.
fn resolve_log_format(cli: &Cli) -> LogFormat {
    cli.log_format.unwrap_or({
        if cli.json {
            LogFormat::Json
        } else {
            LogFormat::Pretty
        }
    })
}

fn init_tracing(verbose: u8, quiet: bool, format: LogFormat) {
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
    let writer = std::io::stderr;
    // The three formats share the same EnvFilter; only the layer's
    // event renderer differs. `target: true` lifts the structured event
    // name (e.g. `decode.orientation.applied`) into the output so
    // grep/jq pipelines can filter on it.
    match format {
        LogFormat::Pretty => {
            registry
                .with(
                    fmt::layer()
                        .with_writer(writer)
                        .with_target(true)
                        .with_timer(fmt::time::uptime()),
                )
                .init();
        },
        LogFormat::Compact => {
            registry
                .with(
                    fmt::layer()
                        .compact()
                        .with_writer(writer)
                        .with_target(true)
                        .with_timer(fmt::time::uptime()),
                )
                .init();
        },
        LogFormat::Json => {
            registry
                .with(fmt::layer().json().with_writer(writer).with_target(true))
                .init();
        },
    }
}

/// Walk the cause chain looking for any `Categorize`-capable error and
/// use its `Category::exit_code()`. Defaults to 1 (Internal) if nothing
/// in the chain implements Categorize.
fn exit_code_for(err: &(dyn std::error::Error + 'static)) -> u8 {
    fn try_downcast(e: &(dyn std::error::Error + 'static)) -> Option<Category> {
        if let Some(p) = e.downcast_ref::<PipelineError>() {
            return Some(p.category());
        }
        if let Some(p) = e.downcast_ref::<photo_frame::DecodeError>() {
            return Some(p.category());
        }
        if let Some(p) = e.downcast_ref::<photo_frame::EncodeError>() {
            return Some(p.category());
        }
        if let Some(p) = e.downcast_ref::<photo_frame::PixelError>() {
            return Some(p.category());
        }
        if let Some(p) = e.downcast_ref::<HexColorError>() {
            // Hex-colour errors are always Input.
            let _ = p;
            return Some(Category::Input);
        }
        if let Some(p) = e.downcast_ref::<CliIoError>() {
            return Some(p.category());
        }
        None
    }

    let mut current: Option<&dyn std::error::Error> = Some(err);
    while let Some(e) = current {
        if let Some(cat) = try_downcast(e) {
            return cat.exit_code();
        }
        current = e.source();
    }
    Category::Internal.exit_code()
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

fn parse_hex_color(s: &str) -> std::result::Result<Background, HexColorError> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return Err(HexColorError::BadLength { got: s.to_owned() });
    }
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|source| HexColorError::BadDigit {
        component: "red",
        source,
    })?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|source| HexColorError::BadDigit {
        component: "green",
        source,
    })?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|source| HexColorError::BadDigit {
        component: "blue",
        source,
    })?;
    Ok(Background::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::{build_options, exit_code_for, parse_hex_color, Cli, CliPreset, HexColorError};
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
        assert!(matches!(
            parse_hex_color("#FFF"),
            Err(HexColorError::BadLength { .. })
        ));
    }

    #[test]
    fn rejects_non_hex() {
        assert!(matches!(
            parse_hex_color("#GGGGGG"),
            Err(HexColorError::BadDigit { .. })
        ));
    }

    #[test]
    fn exit_code_walks_chain_for_pipeline_error() {
        let err = PipelineError::Decode(DecodeError::EmptyInput);
        // 2 = Category::Input::exit_code()
        assert_eq!(exit_code_for(&err), 2);
    }

    #[test]
    fn exit_code_for_decode_failure_is_3() {
        // simulate a decode failure (Category::Decode → 3). Use a
        // fabricated image::ImageError indirectly: the cleanest way is
        // an UnknownFormat (Input → 2) vs HeifFeatureDisabled (also
        // Input → 2), then PixelError → Internal → 1.
        assert_eq!(
            exit_code_for(&PipelineError::Decode(DecodeError::UnknownFormat)),
            2,
        );
        assert_eq!(
            exit_code_for(&PipelineError::Decode(DecodeError::HeifFeatureDisabled)),
            2,
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
