use assert_cmd::Command;
use predicates::prelude::*;

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
fn reports_known_commands_as_not_implemented() {
    let mut command = Command::cargo_bin("nova").expect("nova binary should build");

    command
        .arg("prompt")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "nova: command is not implemented yet",
        ));
}
