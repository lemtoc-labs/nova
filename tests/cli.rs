use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn prints_help_without_a_command() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Usage: nova <init|worker|prompt|check>",
        ))
        .stdout(predicate::str::contains("-V, --version"));
}

#[test]
fn prints_version_from_long_and_short_flags() {
    let expected = format!("nova {}\n", env!("CARGO_PKG_VERSION"));

    for flag in ["--version", "-V"] {
        let mut command = Command::cargo_bin("nova").expect("nova binary should build");

        command
            .arg(flag)
            .assert()
            .success()
            .stdout(expected.clone())
            .stderr(predicate::str::is_empty());
    }
}

#[test]
fn prints_prompt_help_successfully() {
    for flag in ["--help", "-h"] {
        let mut command = Command::cargo_bin("nova").expect("nova binary should build");

        command
            .arg("prompt")
            .arg(flag)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage: nova prompt"))
            .stderr(predicate::str::is_empty());
    }
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
    let config_home = tempfile::tempdir().expect("config home should be created");
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");
    isolate_prompt_env(&mut command, &config_home);

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
    let config_home = tempfile::tempdir().expect("config home should be created");
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");
    isolate_prompt_env(&mut command, &config_home);

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
fn previews_prompt_without_zsh_nonprinting_markers() {
    let config_home = tempfile::tempdir().expect("config home should be created");
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");
    isolate_prompt_env(&mut command, &config_home);

    command
        .arg("prompt")
        .arg("--cwd")
        .arg("/tmp/nova")
        .arg("--format")
        .arg("preview")
        .assert()
        .success()
        .stdout(predicate::str::contains("%{").not())
        .stdout(predicate::str::contains("%}").not())
        .stdout(predicate::str::contains("\u{1b}[32m/tmp/nova"));
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
        .stdout(predicate::str::contains("me"))
        .stdout(predicate::str::contains("@devbox"))
        .stdout(predicate::str::contains("❮"));
}

#[test]
fn renders_configured_user_host_segment_without_ssh() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let config_path = tempdir.path().join("nova.toml");
    fs::write(
        &config_path,
        r#"
        [layout]
        lines = 1

        [layout.line1]
        left = ["user_host", "prompt_char"]
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
        .env("USER", "me")
        .env("HOST", "M4Pro")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .assert()
        .success()
        .stdout(predicate::str::contains("me"))
        .stdout(predicate::str::contains("@M4Pro"));
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

#[test]
fn warns_about_unknown_config_keys_and_segment_tables() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let config_path = tempdir.path().join("nova.toml");
    fs::write(
        &config_path,
        r#"
        [segments.git_status]
        show_count = true

        [segments.git_staus]
        style = { fg = "red" }
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
            "nova: warning: unknown config key `segments.git_status.show_count`",
        ))
        .stderr(predicate::str::contains(
            "nova: warning: unknown segment table `segments.git_staus`",
        ));
}

fn isolate_prompt_env(command: &mut Command, config_home: &tempfile::TempDir) {
    command
        .env_remove("NOVA_CONFIG")
        .env("XDG_CONFIG_HOME", config_home.path());

    for name in [
        "AWSU_PROFILE",
        "AWS_VAULT",
        "AWSUME_PROFILE",
        "AWS_PROFILE",
        "AWS_SSO_PROFILE",
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "AWS_CREDENTIALS_FILE",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "IN_NIX_SHELL",
        "name",
        "NIX_SHELL_LEVEL",
        "VIRTUAL_ENV",
    ] {
        command.env_remove(name);
    }
}
