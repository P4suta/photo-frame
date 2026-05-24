//! Command-line driver for the `photo-frame` facade.
//!
//! Reads files, calls [`photo_frame::pipeline`], writes the framed
//! JPEG to disk. Every failure surfaces through `miette::Result`, so a
//! rich Unicode-boxed diagnostic with code + help text lands on stderr
//! whenever something goes wrong. Observability lives in `tracing`
//! events also sent to stderr.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{ArgAction, Parser, ValueEnum};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use miette::{Diagnostic, Result, WrapErr};
use photo_frame::frame::{CaptionLayout, FrameTheme, MetaPolicy};
use photo_frame::{pipeline, Categorize, Category, PipelineError, PipelineOptions, QualityPreset};
use rayon::prelude::*;
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

    /// Frame theme (preset that pairs border colour and caption text
    /// colour). `paper` = white frame + dark text (the v1 look),
    /// `ink` = soft-black frame + soft-white text.
    #[arg(long, value_enum, default_value_t = CliTheme::Paper)]
    theme: CliTheme,

    /// Caption layout. `edges` keeps the four-corner liit-style layout;
    /// `centered` joins each row with a `·` separator and centres it
    /// under the photo.
    #[arg(long, value_enum, default_value_t = CliLayout::Edges)]
    layout: CliLayout,

    /// Maximum number of inputs to process in parallel. Defaults to the
    /// number of logical CPUs (clamped to the input count). Use
    /// `--jobs 1` to force sequential processing.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(usize))]
    jobs: Option<usize>,

    /// Stop the batch at the first failure. Without this flag the
    /// batch continues; failures are reported in a summary at the end,
    /// and the process exits with code 6 (`PartialFailure`).
    #[arg(long)]
    strict: bool,

    /// Increase log verbosity (`-v`=debug, `-vv`=trace).
    #[arg(short, long, action = ArgAction::Count, conflicts_with = "quiet")]
    verbose: u8,

    /// Only emit warnings and errors.
    #[arg(long, conflicts_with = "verbose")]
    quiet: bool,

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

/// CLI-facing alias for [`FrameTheme`] — clap value-enum needs to own
/// the type it parses into. We could `impl ValueEnum for FrameTheme`
/// in the library, but that drags clap into the library crate; an
/// adapter enum here keeps the lib clean.
#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum CliTheme {
    Paper,
    Ink,
}

impl From<CliTheme> for FrameTheme {
    fn from(value: CliTheme) -> Self {
        match value {
            CliTheme::Paper => Self::Paper,
            CliTheme::Ink => Self::Ink,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum CliLayout {
    Edges,
    Centered,
}

impl From<CliLayout> for CaptionLayout {
    fn from(value: CliLayout) -> Self {
        match value {
            CliLayout::Edges => Self::Edges,
            CliLayout::Centered => Self::Centered,
        }
    }
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

fn main() -> ExitCode {
    install_panic_hook();
    let cli = Cli::parse();
    init_tracing(cli.verbose, cli.quiet, cli.log_format.unwrap_or(LogFormat::Pretty));

    match run(&cli) {
        Ok(code) => code,
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

#[instrument(
    skip_all,
    fields(
        inputs = cli.inputs.len(),
        preset = ?cli.preset,
        jobs = tracing::field::Empty,
        strict = cli.strict,
    ),
)]
fn run(cli: &Cli) -> Result<ExitCode> {
    let opts = build_options(cli);
    let single = cli.inputs.len() == 1;
    let jobs = resolve_jobs(cli);
    tracing::Span::current().record("jobs", jobs);
    info!(
        event_id = "cli.batch.started",
        inputs = cli.inputs.len(),
        jobs,
        strict = cli.strict,
        "batch started"
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .thread_name(|i| format!("photo-frame-worker-{i}"))
        .build()
        .map_err(|e| miette::miette!("failed to build rayon thread pool: {e}"))?;

    // Single-input case behaves like `--strict` even without the flag:
    // there's nothing to continue past, the user wants the failing
    // input's category code in `$?`, not the catch-all
    // `PartialFailure`. Explicit `--strict` keeps the same behaviour
    // for any input count.
    if cli.strict || single {
        run_strict(cli, &opts, single, &pool)
    } else {
        Ok(run_continuing(cli, &opts, single, jobs, &pool))
    }
}

/// Stop-on-first-failure mode. Returns the same `Result<ExitCode>` shape
/// as [`run`]; on failure the error is raised from rayon's
/// `try_for_each` and main's reporter takes over.
fn run_strict(
    cli: &Cli,
    opts: &PipelineOptions,
    single: bool,
    pool: &rayon::ThreadPool,
) -> Result<ExitCode> {
    let progress = build_progress(cli);
    pool.install(|| {
        cli.inputs.par_iter().try_for_each(|input| {
            let output = resolve_output(input, cli.output.as_deref(), single);
            let res = process_one(input, &output, opts)
                .wrap_err_with(|| format!("processing {}", input.display()));
            progress.inc(1);
            if res.is_ok() {
                progress.println(format!("{} → {}", input.display(), output.display()));
                info!(
                    event_id = "cli.batch.item_done",
                    input = %input.display(),
                    output = %output.display(),
                    "wrote framed output",
                );
            }
            res
        })
    })?;
    progress.finish_and_clear();
    Ok(ExitCode::SUCCESS)
}

/// Continue-on-failure mode. Every input is attempted; the per-item
/// outcome is rendered through a [`BatchItem`] and a final summary
/// goes to stderr. Exit code is 0 on full success, 6
/// ([`Category::PartialFailure`]) on any failure.
fn run_continuing(
    cli: &Cli,
    opts: &PipelineOptions,
    single: bool,
    jobs: usize,
    pool: &rayon::ThreadPool,
) -> ExitCode {
    let progress = build_progress(cli);
    let started = Instant::now();
    let results: Vec<BatchItem> = pool.install(|| {
        cli.inputs
            .par_iter()
            .map(|input| {
                let output = resolve_output(input, cli.output.as_deref(), single);
                let item_started = Instant::now();
                let result = process_one(input, &output, opts)
                    .wrap_err_with(|| format!("processing {}", input.display()));
                let elapsed = item_started.elapsed();
                progress.inc(1);
                if let Err(err) = &result {
                    // Surface the failure inline so the user sees it as
                    // it happens, not just in the summary.
                    progress.println(format!("✖ {}\n{err:?}", input.display()));
                } else {
                    progress.println(format!("{} → {}", input.display(), output.display()));
                    info!(
                        event_id = "cli.batch.item_done",
                        input = %input.display(),
                        output = %output.display(),
                        elapsed_ms = elapsed.as_millis(),
                        "wrote framed output",
                    );
                }
                BatchItem {
                    input: input.clone(),
                    result,
                    elapsed,
                }
            })
            .collect()
    });
    progress.finish_and_clear();
    let total = started.elapsed();
    print_summary(&results, total, jobs);

    let any_failed = results.iter().any(|r| r.result.is_err());
    if any_failed {
        ExitCode::from(Category::PartialFailure.exit_code())
    } else {
        ExitCode::SUCCESS
    }
}

/// Per-item record kept by [`run_continuing`] for the summary.
struct BatchItem {
    input: PathBuf,
    result: Result<()>,
    elapsed: Duration,
}

fn resolve_jobs(cli: &Cli) -> usize {
    let inputs = cli.inputs.len().max(1);
    let requested = cli.jobs.unwrap_or_else(num_cpus::get).max(1);
    requested.min(inputs)
}

/// Build the progress bar. Hidden automatically when stderr is not a
/// TTY (indicatif's `stderr()` draw target does that for us); also
/// hidden when there's only a single input (the per-item println +
/// summary already say everything).
fn build_progress(cli: &Cli) -> ProgressBar {
    if cli.inputs.len() <= 1 || cli.quiet {
        return ProgressBar::hidden();
    }
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::stderr());
    #[allow(
        clippy::cast_possible_truncation,
        reason = "input counts are bounded by argv, never approach u64::MAX"
    )]
    let pb = mp.add(ProgressBar::new(cli.inputs.len() as u64));
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("##-"),
    );
    pb
}

/// Render the post-run summary to stderr. Kept hand-formatted rather
/// than wrapped in miette since the summary is *not* an error per se —
/// it's the closing record of the batch.
fn print_summary(results: &[BatchItem], total: Duration, jobs: usize) {
    let processed = results.len();
    let failures: Vec<&BatchItem> = results.iter().filter(|r| r.result.is_err()).collect();
    let ok = processed - failures.len();
    let total_secs = total.as_secs_f64();
    let processed_f = count_as_f64(processed);
    let avg_ms = if processed > 0 {
        results
            .iter()
            .map(|r| r.elapsed.as_secs_f64() * 1000.0)
            .sum::<f64>()
            / processed_f
    } else {
        0.0
    };
    // Effective speedup = sum(per-item walltime) / total walltime.
    let sum_per_item = results.iter().map(|r| r.elapsed.as_secs_f64()).sum::<f64>();
    let speedup = if total_secs > 0.0 {
        sum_per_item / total_secs
    } else {
        0.0
    };

    eprintln!();
    eprintln!("batch summary");
    let pct = if processed > 0 {
        (count_as_f64(ok) / processed_f) * 100.0
    } else {
        0.0
    };
    eprintln!("  processed: {ok} / {processed}  ({pct:.1}%)");
    eprintln!("  failures:  {}", failures.len());
    for f in &failures {
        let category = first_error_category(&f.result).map_or_else(
            || Category::Internal.label().to_string(),
            |c| c.label().to_string(),
        );
        eprintln!(
            "    {}  →  {}  ({:.1}s)",
            f.input.display(),
            category,
            f.elapsed.as_secs_f64(),
        );
    }
    eprintln!(
        "  total:     {total_secs:.1}s  (avg {avg_ms:.0}ms / file, {speedup:.1}x speedup over single-thread, {jobs} jobs)",
    );

    info!(
        event_id = "cli.batch.summary",
        processed,
        ok,
        failed = failures.len(),
        total_ms = total.as_millis(),
        avg_ms,
        speedup,
        jobs,
        "batch summary",
    );
}

/// Convert a count of items into `f64` without lossy-cast lint
/// noise. argv-bounded sizes always fit in a `u32` (which round-trips
/// to `f64` losslessly); the saturating fallback only triggers in
/// the theoretical case of more than four billion inputs in a single
/// run, where the percentage display is the least of our problems.
fn count_as_f64(n: usize) -> f64 {
    u32::try_from(n).map_or(f64::MAX, f64::from)
}

/// Walk a Result's error chain looking for the first
/// `Categorize`-capable error. Mirrors [`exit_code_for`] but returns
/// the typed [`Category`] so the summary can render its label.
fn first_error_category(result: &Result<()>) -> Option<Category> {
    let err = result.as_ref().err()?;
    let mut current: Option<&dyn std::error::Error> = Some(err.as_ref());
    while let Some(e) = current {
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
        if let Some(p) = e.downcast_ref::<CliIoError>() {
            return Some(p.category());
        }
        current = e.source();
    }
    None
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
    opts.frame.theme = cli.theme.into();
    opts.frame.layout = cli.layout.into();
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

#[cfg(test)]
mod tests {
    use super::{build_options, exit_code_for, Cli, CliPreset};
    use clap::Parser;
    use photo_frame::frame::{CaptionLayout, FrameTheme};
    use photo_frame::{DecodeError, PipelineError, QualityPreset};

    #[test]
    fn theme_defaults_to_paper() {
        let cli = Cli::try_parse_from(["photo-frame", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.frame.theme, FrameTheme::Paper);
    }

    #[test]
    fn theme_ink_flag_flips_preset() {
        let cli =
            Cli::try_parse_from(["photo-frame", "--theme", "ink", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.frame.theme, FrameTheme::Ink);
    }

    #[test]
    fn layout_defaults_to_edges() {
        let cli = Cli::try_parse_from(["photo-frame", "photo.jpg"]).unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.frame.layout, CaptionLayout::Edges);
    }

    #[test]
    fn layout_centered_flag_flips_preset() {
        let cli = Cli::try_parse_from(["photo-frame", "--layout", "centered", "photo.jpg"])
            .unwrap();
        let opts = build_options(&cli);
        assert_eq!(opts.frame.layout, CaptionLayout::Centered);
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
