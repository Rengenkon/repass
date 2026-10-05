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
    for (kind, separator) in [
        ("none", None),
        ("between-parts", Some("::")),
        ("between-parts", Some("🟠")),
    ] {
        let mut command = cli();
        command.arg("generate");
        if let Some(separator) = separator {
            command.args(["--separator", separator]);
        }
        let output = command
            .args([
                "--length",
                "13",
                "--count",
                "8",
                "--separator-kind",
                kind,
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
        vec!["generate", "--length", "13"],
        vec!["generate", "--separator-kind", "none"],
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
fn all_separator_names_and_numbers_generate_with_exact_length() {
    let dictionary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unicode.txt");
    for (name, number) in [
        ("none", "1"),
        ("between-parts", "2"),
        ("fixed-interval", "3"),
        ("fixed-count", "4"),
    ] {
        for kind in [name, number] {
            let mut command = cli();
            command
                .args([
                    "generate",
                    "--length",
                    "13",
                    "--count",
                    "4",
                    "--separator-kind",
                    kind,
                    "--dictionary",
                ])
                .arg(&dictionary);
            if name != "none" {
                command.args(["--separator", "🟠::"]);
            }
            // 13 output scalars: interval 3 gives 7 content + 2 * 3 separator scalars.
            if name == "fixed-interval" {
                command.args(["--separator-interval", "3"]);
            }
            if name == "fixed-count" {
                command.args(["--separator-count", "2"]);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{kind}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            assert_eq!(text.lines().count(), 4);
            assert!(text.lines().all(|password| password.chars().count() == 13));
            let count = text.lines().next().unwrap().matches("🟠::").count();
            if name == "fixed-count" || name == "fixed-interval" {
                assert_eq!(count, 2);
            }
        }
    }
}

#[test]
fn invalid_separator_options_are_rejected() {
    for options in [
        vec!["--separator-kind", "0"],
        vec!["--separator-kind", "5"],
        vec!["--separator-kind", "unknown"],
        vec!["--separator-kind", "none", "--separator", "-"],
        vec!["--separator-kind", "between-parts", "--separator", ""],
        vec!["--separator-kind", "none", "--separator-interval", "2"],
        vec![
            "--separator-kind",
            "fixed-interval",
            "--separator-count",
            "2",
        ],
        vec![
            "--separator-kind",
            "fixed-count",
            "--separator-interval",
            "2",
        ],
        vec![
            "--separator-kind",
            "fixed-interval",
            "--separator-interval",
            "0",
        ],
        vec!["--separator-kind", "fixed-count", "--separator-count", "0"],
    ] {
        let output = cli()
            .args(["generate", "--length", "13"])
            .args(&options)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{options:?}");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn separator_help_is_numbered_and_descriptive() {
    for flag in ["--help", "-h"] {
        let output = cli().args(["generate", flag]).output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        for choice in [
            "1. none",
            "2. between-parts",
            "3. fixed-interval",
            "4. fixed-count",
        ] {
            assert!(text.contains(choice), "{text}");
        }
        assert!(text.contains("between dictionary entries"));
        assert!(text.contains("Unicode scalar values"));
        assert!(text.contains("--separator-interval"));
        assert!(text.contains("--separator-count"));
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
fn explicit_interactive_entrypoint_prompts_and_continues_after_storage_errors() {
    for quit in ["q", "quit"] {
        let mut child = cli()
            .arg("interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                format!(
                    "record list\ngenerate\n7\n1\nvault switch next-vault\nvault info\n{quit}\n"
                )
                .as_bytes(),
            )
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

#[test]
fn no_command_prints_help_without_starting_or_resolving_a_session() {
    let output = cli()
        .env_remove("HOME")
        .env("REPASS_DATA_DIR", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Usage:"));
    assert!(text.contains("interactive"));
    assert!(!text.contains("repass>"));
    assert!(!text.contains("Interactive mode."));
}
