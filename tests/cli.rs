use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_cli_insert_and_select() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("sqlajt").unwrap();
    cmd.arg(temp_file.path());

    let input = "insert 1 tester tester@testing.org\nselect\n.exit\n";

    cmd.write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("(1, tester, tester@testing.org)"));
}

#[test]
fn test_cli_negative_id() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("sqlajt").unwrap();
    cmd.arg(temp_file.path());

    let input = "insert -1 tester tester@testing.org\n.exit\n";

    cmd.write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("ID must be a positive integer"));
}

#[test]
fn test_cli_syntax_error() {
    let temp_file = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("sqlajt").unwrap();
    cmd.arg(temp_file.path());

    let input = "insert 1 tester\n.exit\n";

    cmd.write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Syntax error. Expected: insert <id> <username> <email>",
        ));
}
