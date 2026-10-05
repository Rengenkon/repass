use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_repass"));
    command
        .env("HOME", "/unused-home")
        .env_remove("REPASS_DATA_DIR")
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("CLICOLOR");
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
fn long_password_generation_and_selected_presets_work() {
    let output = cli()
        .args([
            "generate",
            "--length",
            "500",
            "--separator-kind",
            "none",
            "--preset",
            "digits",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let password = String::from_utf8(output.stdout).unwrap();
    assert_eq!(password.trim_end().len(), 500);
    assert!(password.trim_end().chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn inclusive_ranges_and_shape_modes_are_available_in_cli() {
    let output = cli()
        .args([
            "generate",
            "--min-length",
            "8",
            "--max-length",
            "12",
            "--count",
            "20",
            "--separator-kind",
            "none",
            "--shape-selection",
            "random",
            "--preset",
            "digits",
            "lowercase",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 20);
    assert!(text.lines().all(|p| {
        (8..=12).contains(&p.len())
            && p.chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    }));
}

#[test]
fn invalid_ranges_conflicting_dictionary_options_and_oversized_batches_fail() {
    for options in [
        vec!["--min-length", "8"],
        vec!["--max-length", "12"],
        vec!["--min-length", "12", "--max-length", "8"],
        vec!["--length", "8", "--min-length", "8", "--max-length", "12"],
        vec![
            "--length",
            "8",
            "--dictionary",
            "unused.txt",
            "--preset",
            "digits",
        ],
        vec!["--length", "8", "--count", "100001"],
        vec!["--length", "1000001"],
    ] {
        let output = cli()
            .args(["generate", "--separator-kind", "none"])
            .args(&options)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{options:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn one_shot_missing_arguments_and_interactive_only_switch_are_errors() {
    for arguments in [
        vec!["generate"],
        vec!["generate", "--length", "13"],
        vec!["generate", "--separator-kind", "none"],
        vec!["generate", "--length", "0"],
        vec!["record", "add"],
        vec!["vault", "switch", "other"],
    ] {
        let output = cli().args(arguments).stdin(Stdio::null()).output().unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("repass>"));
    }
}

#[test]
fn update_clear_flags_conflict_with_setting_the_same_optional_field() {
    for field in ["username", "host", "notes"] {
        let clear_flag = format!("--clear-{field}");
        let set_flag = format!("--{field}");
        let output = cli()
            .args(["record", "update", "1"])
            .arg(&clear_flag)
            .arg(&set_flag)
            .arg("value")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{clear_flag} and {set_flag}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
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
        vec!["--separator-kind", "between-parts", "--separator", ""],
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
fn ignored_separator_options_warn_by_default_and_can_be_suppressed() {
    let mut command = cli();
    let output = command
        .args([
            "generate",
            "--length",
            "13",
            "--separator-kind",
            "none",
            "--separator",
            "::",
            "--separator-interval",
            "3",
            "--separator-count",
            "2",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .chars()
            .count(),
        13
    );
    let warnings = String::from_utf8(output.stderr).unwrap();
    for option in ["--separator", "--separator-interval", "--separator-count"] {
        assert!(
            warnings.contains(&format!("{option} is ignored")),
            "{warnings}"
        );
    }

    let output = cli()
        .args([
            "generate",
            "--length",
            "13",
            "--separator-kind",
            "none",
            "--separator-count",
            "2",
            "--no-warnings",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .chars()
            .count(),
        13
    );
}

#[test]
fn ignored_options_are_still_type_checked_and_warning_flags_are_exclusive() {
    for arguments in [
        vec![
            "generate",
            "--length",
            "13",
            "--separator-kind",
            "none",
            "--separator-interval",
            "invalid",
        ],
        vec![
            "generate",
            "--length",
            "13",
            "--separator-kind",
            "none",
            "--separator-count",
            "0",
        ],
        vec![
            "generate",
            "--length",
            "13",
            "--separator-kind",
            "none",
            "--warnings",
            "--no-warnings",
        ],
    ] {
        let output = cli().args(arguments).output().unwrap();
        assert!(!output.status.success());
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
fn storage_commands_prompt_for_a_master_password_before_opening() {
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
    assert!(error.contains("Master password:"));
    assert!(!error.contains("environment-vault"));
    assert!(!root.exists());

    let output = cli()
        .env("REPASS_DATA_DIR", "environment-vault")
        .args(["vault", "info"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Master password:")
    );
    let output = cli().args(["vault", "info"]).output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Master password:")
    );
}

#[test]
fn explicit_interactive_entrypoint_continues_after_password_prompt_errors() {
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
        assert!(text.contains("Master password:"));
        assert!(text.contains("Data directory: next-vault"));
    }
}

#[test]
fn interactive_warning_setting_is_inherited_and_can_be_overridden_per_generation() {
    let mut child = cli()
        .args(["interactive", "--no-warnings"])
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
            b"generate --length 13 --separator-kind none --separator-count 2\nwarnings\ngenerate --length 13 --separator-kind none --separator-count 2 --warnings\nwarnings\ngenerate --length 13 --separator-kind none --separator-count 2\nwarnings on\ngenerate --length 13 --separator-kind none --separator-count 2\nwarnings off\ngenerate --length 13 --separator-kind none --separator-count 2\nquit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text.matches("--separator-count is ignored").count(),
        2,
        "{text}"
    );
    assert!(text.contains("Warnings: off"));
    assert!(text.contains("Warnings: on"));
    assert!(text.contains("--separator-count is ignored for separator strategy none"));
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

#[test]
fn data_directory_options_are_scoped_to_storage_commands() {
    for arguments in [vec!["completions", "--help"], vec!["generate", "--help"]] {
        let output = cli().args(arguments).output().unwrap();
        assert!(output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("--data-dir"));
    }

    let output = cli().args(["record", "list", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--data-dir"));
}

#[test]
fn generation_does_not_resolve_a_data_directory() {
    let output = cli()
        .env_remove("HOME")
        .env("REPASS_DATA_DIR", "")
        .args(["generate", "--length", "13", "--separator-kind", "none"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .chars()
            .count(),
        13
    );
}

#[test]
fn shell_completion_scripts_include_commands_flags_and_separator_aliases() {
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = cli()
            .env_remove("HOME")
            .env("REPASS_DATA_DIR", "")
            .env("CLICOLOR_FORCE", "1")
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(!script.contains('\x1b'));
        assert!(!script.contains("TODO:"));
        for item in [
            "repass",
            "generate",
            "record",
            "separator-kind",
            "dictionary",
            "data-dir",
            "between-parts",
            "fixed-count",
            "rename",
            "recover",
        ] {
            assert!(script.contains(item), "missing {item} in {shell} script");
        }
        // Fish/zsh quote individual possible values; Bash uses a word list.
        // Check every generator exposes all aliases, not just canonical names.
        for number in ["1", "2", "3", "4"] {
            assert!(
                script
                    .split(|ch: char| !ch.is_ascii_alphanumeric())
                    .any(|word| word == number),
                "missing alias {number} in {shell} script"
            );
        }
        if shell == "bash" {
            let mut child = Command::new("bash")
                .arg("-n")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(script.as_bytes())
                .unwrap();
            assert!(child.wait().unwrap().success());
        }
    }
    let output = cli().args(["completions", "unknown"]).output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn redirected_output_is_plain_and_no_color_disables_forced_color() {
    for no_color in [false, true] {
        let mut command = cli();
        if no_color {
            command.env("NO_COLOR", "1").env("CLICOLOR_FORCE", "1");
        }
        let output = command.arg("--help").output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));

        let mut command = cli();
        if no_color {
            command.env("NO_COLOR", "1").env("CLICOLOR_FORCE", "1");
        }
        let output = command.args(["vault", "info"]).output().unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.contains(&0x1b));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Master password:")
        );

        let mut command = cli();
        if no_color {
            command.env("NO_COLOR", "1").env("CLICOLOR_FORCE", "1");
        }
        let mut child = command
            .arg("interactive")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"help\ngenerate\n7\n1\nrecord list\nq\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));
    }
}

#[test]
fn forced_color_styles_help_errors_and_session_but_not_generated_passwords() {
    let output = cli()
        .env("CLICOLOR_FORCE", "1")
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.stdout.contains(&0x1b));
    let output = cli()
        .env("CLICOLOR_FORCE", "1")
        .args(["vault", "info"])
        .output()
        .unwrap();
    assert!(output.stderr.contains(&0x1b));
    let output = cli()
        .env("CLICOLOR_FORCE", "1")
        .args(["generate", "--length", "13", "--separator-kind", "none"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .trim_end()
            .chars()
            .count(),
        13
    );
    let mut child = cli()
        .env("CLICOLOR_FORCE", "1")
        .arg("interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"help\ngenerate\n7\n1\nrecord list\nq\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.contains(&0x1b));
}
