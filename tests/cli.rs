use assert_cmd::Command;
use tempfile::NamedTempFile;

#[test]
fn test_cli_syntax_error() {
    let file = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("sqlajt").unwrap();

    cmd.arg(file.path())
        .write_stdin("insert 1 tester\n.exit\n")
        .assert()
        .stdout(predicates::str::contains("Parse Error"));
}

#[test]
fn test_create_insert_select() {
    let file = NamedTempFile::new().unwrap();
    let mut cmd = Command::cargo_bin("sqlajt").unwrap();

    cmd.arg(file.path())
        .write_stdin("CREATE TABLE users (id INTEGER, name VARCHAR(255));\nINSERT INTO users VALUES (1, 'alice');\nSELECT * FROM users;\n.exit\n")
        .assert()
        .stdout(predicates::str::contains("Table users created."))
        .stdout(predicates::str::contains("Inserted 1 row(s)."))
        .stdout(predicates::str::contains("1 | alice"))
        .stdout(predicates::str::contains("1 row(s) returned."));
}

#[test]
fn test_persistence() {
    let file = NamedTempFile::new().unwrap();

    // First session: create and insert
    let mut cmd1 = Command::cargo_bin("sqlajt").unwrap();
    cmd1.arg(file.path())
        .write_stdin("CREATE TABLE users (id INTEGER, name VARCHAR(255));\nINSERT INTO users VALUES (1, 'alice');\n.exit\n")
        .assert()
        .stdout(predicates::str::contains("Table users created."))
        .stdout(predicates::str::contains("Inserted 1 row(s)."));

    // Second session: select
    let mut cmd2 = Command::cargo_bin("sqlajt").unwrap();
    cmd2.arg(file.path())
        .write_stdin("SELECT * FROM users;\n.exit\n")
        .assert()
        .stdout(predicates::str::contains("1 | alice"))
        .stdout(predicates::str::contains("1 row(s) returned."));
}
