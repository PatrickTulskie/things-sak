//! Read-only AppleScript layer for Things 3.
//!
//! All mutations go through the Things URL scheme (see `url_scheme`); this
//! module only queries data. Records are serialized by AppleScript using the
//! ASCII unit/record separator control characters, which cannot realistically
//! appear in todo content, so notes with newlines/tabs survive round-trips.

use anyhow::{Context, Result, bail};
use tokio::process::Command;

pub const FIELD_SEP: char = '\u{1F}';
pub const RECORD_SEP: char = '\u{1E}';

/// AppleScript preamble binding the separator characters to `fs`/`rs`.
pub const SEP_PREAMBLE: &str = "set fs to character id 31\nset rs to character id 30";

pub async fn run(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .await
        .context("failed to spawn osascript — is this running on macOS?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("AppleScript error: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// Wrap a script body in a `tell application "Things3"` block.
pub fn tell_things(body: &str) -> String {
    format!("tell application \"Things3\"\n{body}\nend tell")
}

/// Quote a string for safe embedding in AppleScript source.
pub fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Split osascript output into records of fields.
pub fn parse_records(output: &str) -> Vec<Vec<String>> {
    output
        .split(RECORD_SEP)
        .map(|r| r.trim_matches(['\n', '\r']))
        .filter(|r| !r.is_empty())
        .map(|record| record.split(FIELD_SEP).map(str::to_string).collect())
        .collect()
}

/// Optional field helper: empty string becomes None.
pub fn opt(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_quotes_and_backslashes() {
        assert_eq!(quote("plain"), "\"plain\"");
        assert_eq!(quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(quote(r"a\b"), r#""a\\b""#);
        // Injection attempt: quotes cannot break out of the string literal.
        assert_eq!(
            quote(r#"" & (do shell script "true") & ""#),
            r#""\" & (do shell script \"true\") & \"""#
        );
    }

    #[test]
    fn parse_records_splits_fields_and_records() {
        let output = format!(
            "id1{f}name one{f}open{r}id2{f}name\ntwo{f}completed{r}",
            f = FIELD_SEP,
            r = RECORD_SEP
        );
        let records = parse_records(&output);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], vec!["id1", "name one", "open"]);
        // Newlines inside fields survive.
        assert_eq!(records[1][1], "name\ntwo");
    }

    #[test]
    fn parse_records_handles_empty_output() {
        assert!(parse_records("").is_empty());
        assert!(parse_records("\n").is_empty());
    }
}
