//! Integration checks of the production Git pre-push protocol boundary.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bash() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::split_paths(&std::env::var_os("PATH").expect("tool search path"))
            .find(|directory| {
                directory.join("bash.exe").is_file() && directory.join("sh.exe").is_file()
            })
            .expect("Git Bash installation on PATH")
            .join("bash.exe")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("bash")
    }
}

fn command() -> Command {
    let mut command = Command::new(bash());
    command
        .args(["--noprofile", "--norc"])
        .current_dir(root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
}

fn with_input(mut command: Command, protocol: &[u8]) -> Output {
    let mut child = command.stdin(Stdio::piped()).spawn().expect("start hook");
    child
        .stdin
        .take()
        .expect("protocol pipe")
        .write_all(protocol)
        .expect("write protocol");
    child.wait_with_output().expect("join hook")
}

fn run(protocol: &[u8]) -> Output {
    let mut child = command()
        .arg(root().join("scripts/check-signing.sh"))
        .args(["pre-push", "origin"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("start the production signing script");
    child
        .stdin
        .take()
        .expect("protocol input")
        .write_all(protocol)
        .expect("write the protocol");
    child.wait_with_output().expect("join the signing script")
}

fn head() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root())
        .output()
        .expect("read the checkout commit");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("Git object identifier")
        .trim()
        .to_owned()
}

fn assert_rejected(output: &Output) {
    assert!(
        !output.status.success(),
        "verification unexpectedly passed; stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn closed_protocol_input_is_rejected() {
    let output = command()
        .args([
            "-c",
            "exec \"$BASH\" \"$1\" pre-push origin <&-",
            "signing-test",
        ])
        .arg(root().join("scripts/check-signing.sh"))
        .output()
        .expect("run with a closed input descriptor");
    assert_rejected(&output);
}

#[test]
fn malformed_protocol_is_rejected() {
    assert_rejected(&run(b"missing-fields\n"));
}

#[test]
fn failed_git_query_is_rejected() {
    assert_rejected(&run(
        b"refs/heads/work 1111111111111111111111111111111111111111 refs/heads/work 2222222222222222222222222222222222222222\n",
    ));
}

#[test]
fn invalid_final_record_without_line_feed_is_rejected() {
    assert_rejected(&run(b"missing-fields"));
}

#[test]
fn readable_empty_protocol_is_valid() {
    let output = run(b"");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn valid_range_without_new_commits_is_valid() {
    let head = head();
    let output = run(format!("HEAD {head} refs/heads/work {head}\n").as_bytes());
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn deleted_ref_does_not_require_a_commit_signature() {
    let output = run(format!(
        "(delete) 0000000000000000000000000000000000000000 refs/heads/work {}\n",
        head()
    )
    .as_bytes());
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn nul_and_extra_protocol_fields_are_rejected() {
    let head = head();
    for protocol in [
        format!("HEAD\0 {head} refs/heads/work {head}\n"),
        format!("HEAD {head} refs/heads/work {head} extra\n"),
        format!("HEAD {head} refs/heads/work {head}"),
        format!("HEAD {head} refs/heads/work {head}\n\n"),
    ] {
        assert_rejected(&run(protocol.as_bytes()));
    }
}

#[test]
fn only_good_signature_verdicts_are_accepted() {
    let head = head();
    for verdict in ["G", "N", "B", "U", "X", "Y", "R", "E", "?", "", "G extra"] {
        let mut command = command();
        command
            .args([
                "-c",
                "git() { if [ \"$1\" = log ]; then printf '%s %s\\n' \"$SIGNING_TEST_SHA\" \"$SIGNING_TEST_VERDICT\"; else command git \"$@\"; fi; }; export -f git; exec \"$BASH\" \"$1\" pre-push",
                "signing-test",
            ])
            .arg(root().join("scripts/check-signing.sh"))
            .env("SIGNING_TEST_SHA", &head)
            .env("SIGNING_TEST_VERDICT", verdict);
        let output = with_input(
            command,
            format!("HEAD {head} refs/heads/work {head}\n").as_bytes(),
        );
        assert_eq!(
            output.status.success(),
            verdict == "G",
            "{verdict}: {output:?}"
        );
    }
}

#[derive(Debug)]
struct Fixture {
    directory: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "photo-frame-signing-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).expect("create isolated signing fixture");
        let fixture = Self { directory };
        std::fs::write(fixture.directory.join("gitconfig"), "").expect("empty Git config");
        fixture.git(&["init", "--initial-branch=main", "--template=", "work"]);
        fixture.git(&["-C", "work", "config", "user.name", "Signing fixture"]);
        fixture.git(&[
            "-C",
            "work",
            "config",
            "user.email",
            "fixture@example.invalid",
        ]);
        fixture.commit();
        fixture.git(&["clone", "--bare", "work", "remote"]);
        fixture
    }

    fn isolate(&self, command: &mut Command) {
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.directory.join("gitconfig"));
    }

    fn git(&self, arguments: &[&str]) -> String {
        let mut command = Command::new("git");
        self.isolate(&mut command);
        let output = command
            .args(arguments)
            .current_dir(&self.directory)
            .output()
            .expect("run fixture Git");
        assert!(output.status.success(), "{arguments:?}: {output:?}");
        String::from_utf8(output.stdout)
            .expect("Git text output")
            .trim()
            .to_owned()
    }

    fn commit(&self) {
        self.git(&[
            "-C",
            "work",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "test: create unsigned negative control",
        ]);
    }

    fn configure_signing(&self) {
        let key = self.directory.join("signing-key");
        let output = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&key)
            .output()
            .expect("generate an isolated test key");
        assert!(output.status.success(), "{output:?}");
        let public_key =
            std::fs::read_to_string(key.with_extension("pub")).expect("read the test public key");
        let signers = self.directory.join("allowed-signers");
        std::fs::write(&signers, format!("fixture@example.invalid {public_key}"))
            .expect("write test signers");
        self.git(&["-C", "work", "config", "gpg.format", "ssh"]);
        self.git(&[
            "-C",
            "work",
            "config",
            "user.signingkey",
            key.to_str().expect("key path"),
        ]);
        self.git(&[
            "-C",
            "work",
            "config",
            "gpg.ssh.allowedSignersFile",
            signers.to_str().expect("signers path"),
        ]);
    }

    fn push_check(&self, destination: &str) -> Output {
        let head = self.git(&["-C", "work", "rev-parse", "HEAD"]);
        let mut command = command();
        self.isolate(&mut command);
        command
            .arg(root().join("scripts/check-signing.sh"))
            .arg("pre-push")
            .arg(self.directory.join(destination))
            .current_dir(self.directory.join("work"));
        with_input(
            command,
            format!("HEAD {head} refs/heads/work {}\n", "0".repeat(40)).as_bytes(),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).expect("remove owned signing fixture");
    }
}

#[test]
fn new_branch_excludes_existing_destination_history() {
    let fixture = Fixture::new();
    let output = fixture.push_check("remote");
    assert!(output.status.success(), "{output:?}");
    assert!(!fixture.directory.join("work/.git/FETCH_HEAD").exists());
    assert_eq!(fixture.git(&["-C", "work", "branch", "-r"]), "");
}

#[test]
fn new_unsigned_commit_is_rejected() {
    let fixture = Fixture::new();
    fixture.commit();
    assert_rejected(&fixture.push_check("remote"));
}

#[test]
fn unavailable_destination_is_rejected() {
    let fixture = Fixture::new();
    assert_rejected(&fixture.push_check("missing-remote"));
}

#[test]
fn new_signed_commit_is_valid() {
    let fixture = Fixture::new();
    fixture.configure_signing();
    fixture.git(&[
        "-C",
        "work",
        "commit",
        "-S",
        "--allow-empty",
        "-m",
        "test: sign the new commit",
    ]);
    let output = fixture.push_check("remote");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn signature_display_configuration_preserves_machine_records() {
    let fixture = Fixture::new();
    fixture.configure_signing();
    fixture.git(&[
        "-C",
        "work",
        "commit",
        "-S",
        "--allow-empty",
        "-m",
        "test: sign the new commit",
    ]);
    fixture.git(&["-C", "work", "config", "log.showSignature", "true"]);
    fixture.git(&["-C", "work", "config", "color.ui", "always"]);
    fixture.git(&["-C", "work", "notes", "add", "-m", "unrelated note"]);
    let output = fixture.push_check("remote");
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn replacement_objects_cannot_hide_an_unsigned_commit() {
    let fixture = Fixture::new();
    let base = fixture.git(&["-C", "work", "rev-parse", "HEAD"]);
    fixture.commit();
    let unsigned = fixture.git(&["-C", "work", "rev-parse", "HEAD"]);
    let tree = fixture.git(&["-C", "work", "rev-parse", "HEAD:"]);
    fixture.configure_signing();
    let replacement = fixture.git(&[
        "-C",
        "work",
        "commit-tree",
        "-S",
        &tree,
        "-p",
        &base,
        "-m",
        "test: sign a replacement object",
    ]);
    fixture.git(&["-C", "work", "replace", &unsigned, &replacement]);
    assert_eq!(
        fixture.git(&["-C", "work", "log", "-1", "--format=%G?", &unsigned]),
        "G"
    );
    assert_rejected(&fixture.push_check("remote"));
}

#[test]
fn snapshot_fetch_preserves_local_tags_with_pruning_enabled() {
    let fixture = Fixture::new();
    fixture.git(&["-C", "work", "config", "fetch.prune", "true"]);
    fixture.git(&["-C", "work", "config", "fetch.pruneTags", "true"]);
    fixture.git(&["-C", "work", "tag", "local-only"]);
    let output = fixture.push_check("remote");
    assert!(output.status.success(), "{output:?}");
    fixture.git(&["-C", "work", "show-ref", "--verify", "refs/tags/local-only"]);
}

#[test]
fn unreadable_directory_input_is_rejected() {
    let fixture = Fixture::new();
    let output = command()
        .args([
            "-c",
            "exec \"$BASH\" \"$1\" pre-push <\"$2\"",
            "signing-test",
        ])
        .arg(root().join("scripts/check-signing.sh"))
        .arg(&fixture.directory)
        .output()
        .expect("run with directory input");
    assert_rejected(&output);
}
