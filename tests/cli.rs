use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn prints_help_without_a_command() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command.assert().success().stdout(predicate::str::contains(
        "Usage: nova <init|worker|prompt|check>",
    ));
}

#[test]
fn rejects_unknown_commands() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("unknown")
        .assert()
        .failure()
        .stderr(predicate::str::contains("nova: unknown command `unknown`"))
        .stdout(predicate::str::contains(
            "Usage: nova <init|worker|prompt|check>",
        ));
}

#[test]
fn renders_zsh_init_script() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("init")
        .arg("zsh")
        .assert()
        .success()
        .stdout(predicate::str::contains("add-zsh-hook precmd _nova_precmd"))
        .stdout(predicate::str::contains("typeset -g _nova_bin='"));
}

#[test]
fn renders_prompt_command() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("prompt")
        .arg("--cwd")
        .arg("/tmp/nova")
        .arg("--cols")
        .arg("40")
        .arg("--exit")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("/tmp/nova"))
        .stdout(predicate::str::contains("[1]"))
        .stdout(predicate::str::contains("❯"));
}

#[test]
fn rejects_zero_columns() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("prompt")
        .arg("--cols")
        .arg("0")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--cols must be greater than 0"));
}

#[test]
fn prints_shell_assignments_for_prompt_command() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("prompt")
        .arg("--cwd")
        .arg("/tmp/nova")
        .arg("--format")
        .arg("shell")
        .assert()
        .success()
        .stdout(predicate::str::contains("PROMPT='"))
        .stdout(predicate::str::contains("RPROMPT='"));
}

#[test]
fn renders_configured_ssh_segment() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let config_path = tempdir.path().join("nova.toml");
    fs::write(
        &config_path,
        r#"
        [layout]
        lines = 1

        [layout.line1]
        left = ["ssh", "prompt_char"]
        right = []
        "#,
    )
    .expect("config should be written");

    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("prompt")
        .arg("--config")
        .arg(config_path)
        .arg("--cwd")
        .arg("/tmp/nova")
        .arg("--keymap")
        .arg("vicmd")
        .env("SSH_CONNECTION", "127.0.0.1 1 127.0.0.1 2")
        .env("USER", "me")
        .env("HOSTNAME", "devbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("me@devbox"))
        .stdout(predicate::str::contains("❮"));
}

#[test]
fn validates_config_files() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let config_path = tempdir.path().join("nova.toml");
    fs::write(&config_path, "[layout]\nlines = 1\n").expect("config should be written");

    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("check")
        .arg("--config")
        .arg(config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("nova: config ok"));
}

#[test]
fn warns_about_unknown_segments_while_validating_config_files() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let config_path = tempdir.path().join("nova.toml");
    fs::write(
        &config_path,
        r#"
        [layout]
        lines = 1

        [layout.line1]
        left = ["dir", "missing"]
        right = []
        "#,
    )
    .expect("config should be written");

    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("check")
        .arg("--config")
        .arg(config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("nova: config ok"))
        .stderr(predicate::str::contains(
            "nova: warning: unknown segment `missing` in `layout.line1.left`",
        ));
}
