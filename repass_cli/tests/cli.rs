use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_repass"));
    command
        .env("HOME", "/unused-home")
        .env_remove("REPASS_DATA_DIR");
    command
}

#[test]
fn generation_respects_exact_unicode_length_and_count() {
    let dictionary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unicode.txt");
    for separator in ["", "::", "🟠"] {
        let output = cli()
            .args([
                "generate",
                "--length",
                "13",
                "--count",
                "8",
                "--separator",
                separator,
                "--dictionary",
            ])
            .arg(&dictionary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), 8);
        assert!(text.lines().all(|password| password.chars().count() == 13));
    }
}

#[test]
fn one_shot_missing_arguments_and_interactive_only_switch_are_errors() {
    for arguments in [
        vec!["generate"],
        vec!["generate", "--length", "0"],
        vec!["record", "add", "--name", "mail"],
        vec!["vault", "switch", "other"],
    ] {
        let output = cli().args(arguments).stdin(Stdio::null()).output().unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("repass>"));
    }
}

#[test]
fn directory_precedence_is_used_without_creating_storage() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/nonexistent-vault");
    assert!(!root.exists());
    let output = cli()
        .env("REPASS_DATA_DIR", "environment-vault")
        .args(["record", "list", "--data-dir"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains(root.to_str().unwrap()));
    assert!(!error.contains("environment-vault"));
    assert!(!root.exists());

    let output = cli()
        .env("REPASS_DATA_DIR", "environment-vault")
        .args(["vault", "info"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("directory: environment-vault")
    );
    let output = cli().args(["vault", "info"]).output().unwrap();
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("directory: /unused-home/.repass")
    );
}

#[test]
fn both_interactive_entrypoints_prompt_and_continue_after_storage_errors() {
    for arguments in [vec![], vec!["interactive"]] {
        let mut child = cli()
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"record list\ngenerate\n7\nvault switch next-vault\nvault info\nexit\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("length: "));
        assert!(text.contains("TODO: repass_storage"));
        assert!(text.contains("directory: next-vault"));
        assert!(text.contains("Data directory: next-vault"));
    }
}
