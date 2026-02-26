use assert_cmd::prelude::*;
use assert_cmd::cargo::cargo_bin_cmd;

/// Tests that `--help` is handled successfully by the CLI.
///
/// This test verifies:
/// 1. Running `hippocampus-cli --help` exits successfully
/// 2. The help text is written to stdout (captured and printed for visibility)
/// 3. No unexpected stderr output is produced
#[test]
fn test_cli_help_success() {
  let mut cmd = cargo_bin_cmd!("hippocampus-cli");

  let assert = cmd.arg("--help").assert().success();

  let out = assert.get_output();
  println!("=== hippocampus-cli --help stdout ===\n\n{}\n=====================================", String::from_utf8_lossy(&out.stdout));

  assert!(
  	!out.stdout.is_empty(),
  	"expected non-empty stdout for --help"
  );
  assert!(
  	out.stderr.is_empty(),
  	"expected empty stderr for --help, got:\n{}",
  	String::from_utf8_lossy(&out.stderr)
  );
}
