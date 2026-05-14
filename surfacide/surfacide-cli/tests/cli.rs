//! CLI integration tests via trycmd.
//!
//! Each `.toml` file under `tests/trycmd/` is a golden-file CLI session.
//! These tests assert on stdout/stderr/exit code; they treat the
//! diagnostic shape as part of the public CLI API. Update goldens when
//! you intentionally change diagnostic wording or codes.

#[test]
fn cli_trycmd() {
    trycmd::TestCases::new()
        .case("tests/trycmd/*.toml");
}
