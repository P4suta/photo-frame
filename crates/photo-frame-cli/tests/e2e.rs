//! End-to-end CLI tests.
//!
//! These tests compile and invoke the `photo-frame` binary as a real
//! subprocess so the full clap → pipeline → file-write path runs. Unit
//! tests cover module-internal logic; this suite covers the binary's
//! externally observable contract — stdout / stderr / exit code /
//! output file shape.

use assert_cmd::Command;
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder, RgbImage};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::TempDir;

/// Build a small synthetic JPEG in `tempdir/<name>.jpg` and return its path.
fn synth_jpeg(dir: &TempDir, name: &str, w: u32, h: u32) -> PathBuf {
    let img = RgbImage::from_pixel(w, h, image::Rgb([200, 60, 60]));
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 90)
        .write_image(&img, w, h, ExtendedColorType::Rgb8)
        .expect("synthetic jpeg encode");
    let path = dir.path().join(format!("{name}.jpg"));
    std::fs::write(&path, &bytes).expect("write tempfile");
    path
}

fn cli() -> Command {
    Command::cargo_bin("photo-frame").expect("photo-frame binary built")
}

#[test]
fn cli_processes_synthesized_jpeg() {
    let dir = TempDir::new().expect("tempdir");
    let input = synth_jpeg(&dir, "in", 200, 150);
    let output = dir.path().join("out.jpg");

    cli()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();

    let out_bytes = std::fs::read(&output).expect("output written");
    assert!(!out_bytes.is_empty(), "output JPEG must have nonzero size");
    // JPEG SOI marker
    assert_eq!(
        &out_bytes[..2],
        &[0xFF, 0xD8],
        "output must start with JPEG SOI"
    );
}

#[test]
fn cli_rejects_empty_input() {
    cli()
        .arg("/dev/null")
        .arg("--quiet")
        .assert()
        // Category::Input.exit_code() = 2
        .code(2)
        .stderr(contains("empty_input").or(contains("input is empty")));
}

#[test]
fn cli_rejects_missing_input_with_input_category_exit() {
    cli()
        .arg("/tmp/photo_frame_e2e_nonexistent_input.jpg")
        .arg("--quiet")
        .assert()
        // CliIoError::ReadInput → Category::Input → 2
        .code(2)
        .stderr(contains("io::read").or(contains("No such file")));
}

#[test]
fn cli_emits_structured_json_log() {
    let dir = TempDir::new().expect("tempdir");
    let input = synth_jpeg(&dir, "json", 64, 64);
    let output = dir.path().join("json_out.jpg");

    let assertion = cli()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--log-format")
        .arg("json")
        .assert()
        .success();

    let stderr = std::str::from_utf8(&assertion.get_output().stderr).expect("stderr is utf-8");
    // Every non-empty line should parse as JSON; just sample the first
    // non-empty line to keep the assertion sturdy under future
    // tracing-subscriber tweaks.
    let first_line = stderr
        .lines()
        .find(|l| !l.trim().is_empty())
        .expect("at least one log line on success");
    let parsed: serde_json::Value = serde_json::from_str(first_line)
        .unwrap_or_else(|e| panic!("first stderr line must be JSON: {e}\nline = {first_line}"));
    // A tracing JSON event has `level` + `fields` keys.
    assert!(
        parsed.get("level").is_some(),
        "JSON event must have a `level` field"
    );
    assert!(
        parsed.get("fields").is_some(),
        "JSON event must have a `fields` object"
    );
}

#[test]
fn cli_help_lists_log_format_flag() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("--log-format"))
        .stdout(contains("pretty"))
        .stdout(contains("json"));
}

#[test]
fn cli_theme_unknown_returns_clap_usage_error() {
    // clap rejects unknown enum values before run() — exit code is 2
    // (clap's standard "usage error" convention).
    cli()
        .arg("/tmp/anything.jpg")
        .arg("--theme")
        .arg("midnight")
        .assert()
        .code(2);
}

#[test]
fn cli_batch_3_inputs_all_succeed() {
    let dir = TempDir::new().expect("tempdir");
    let inputs: Vec<PathBuf> = (0..3)
        .map(|i| synth_jpeg(&dir, &format!("ok_{i}"), 80, 60))
        .collect();
    let out_dir = dir.path().join("out");
    std::fs::create_dir(&out_dir).expect("mkdir out");

    let mut cmd = cli();
    for p in &inputs {
        cmd.arg(p);
    }
    cmd.arg("-o")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        .success();

    // Three framed JPEGs land in out_dir.
    let produced: Vec<_> = std::fs::read_dir(&out_dir)
        .expect("readdir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(produced.len(), 3, "expected three framed outputs in {out_dir:?}");
}

#[test]
fn cli_batch_continues_on_partial_failure_and_summarises() {
    let dir = TempDir::new().expect("tempdir");
    let ok_a = synth_jpeg(&dir, "ok_a", 80, 60);
    let ok_b = synth_jpeg(&dir, "ok_b", 80, 60);
    let bad = dir.path().join("bad.jpg");
    std::fs::write(&bad, b"not an image").expect("write garbage");
    let out_dir = dir.path().join("out");
    std::fs::create_dir(&out_dir).expect("mkdir out");

    cli()
        .arg(&ok_a)
        .arg(&bad)
        .arg(&ok_b)
        .arg("-o")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        // PartialFailure → 6.
        .code(6)
        .stderr(contains("batch summary"))
        .stderr(contains("failures:  1"));

    // The two good inputs still produced framed outputs.
    let produced: Vec<_> = std::fs::read_dir(&out_dir)
        .expect("readdir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(
        produced.len(),
        2,
        "two successful inputs should leave two outputs, got {produced:?}",
    );
}

#[test]
fn cli_batch_strict_stops_on_first_failure() {
    let dir = TempDir::new().expect("tempdir");
    let bad = dir.path().join("bad.jpg");
    std::fs::write(&bad, b"not an image").expect("write garbage");
    let ok = synth_jpeg(&dir, "ok", 80, 60);
    let out_dir = dir.path().join("out");
    std::fs::create_dir(&out_dir).expect("mkdir out");

    // With --strict, the first failure surfaces *that* error's
    // category code, not the catch-all PartialFailure (6). "Not an
    // image" maps to `DecodeError::UnknownFormat` which classifies as
    // Category::Input (the bytes don't even resemble a supported
    // format) → exit 2.
    cli()
        .arg(&bad)
        .arg(&ok)
        .arg("-o")
        .arg(&out_dir)
        .arg("--strict")
        .arg("--jobs")
        .arg("1")
        .arg("--quiet")
        .assert()
        .code(2);
}

#[test]
fn cli_batch_jobs_1_runs_sequentially() {
    // Smoke test: --jobs 1 must still succeed (we're not racing the
    // process for timing here, only verifying the path doesn't panic
    // and exit code is correct).
    let dir = TempDir::new().expect("tempdir");
    let inputs: Vec<PathBuf> = (0..2)
        .map(|i| synth_jpeg(&dir, &format!("seq_{i}"), 64, 48))
        .collect();
    let out_dir = dir.path().join("out");
    std::fs::create_dir(&out_dir).expect("mkdir out");

    let mut cmd = cli();
    for p in &inputs {
        cmd.arg(p);
    }
    cmd.arg("-o")
        .arg(&out_dir)
        .arg("--jobs")
        .arg("1")
        .arg("--quiet")
        .assert()
        .success();
}
