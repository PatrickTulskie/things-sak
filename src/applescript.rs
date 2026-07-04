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

    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
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
    if s.is_empty() { None } else { Some(s.to_string()) }
}
