//! Smoke test: the binary prints its name and version and exits successfully.

use std::process::Command;

#[test]
fn prints_name_and_version() {
    let output = match Command::new(env!("CARGO_BIN_EXE_pathmuster")).output() {
        Ok(output) => output,
        Err(error) => panic!("failed to run pathmuster: {error}"),
    };
    assert!(
        output.status.success(),
        "pathmuster exited with {}",
        output.status
    );
    let stdout = match String::from_utf8(output.stdout) {
        Ok(stdout) => stdout,
        Err(error) => panic!("stdout is not UTF-8: {error}"),
    };
    assert_eq!(stdout, format!("pathmuster {}\n", pathmuster::version()));
}
