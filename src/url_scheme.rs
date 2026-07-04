//! Things URL scheme write layer.
//!
//! All mutations use the official `things:///` URL scheme
//! (https://culturedcode.com/things/support/articles/2803573/) rather than
//! touching the Things database. `update` and `update-project` require the
//! auth token from Things → Settings → General → Enable Things URLs.

use anyhow::{Context, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use tokio::process::Command;

pub const AUTH_TOKEN_ENV: &str = "THINGS_SAK_AUTH_TOKEN";

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

pub fn build_url(command: &str, params: &[(&str, &str)]) -> String {
    let mut url = format!("things:///{command}");
    let mut first = true;
    for (key, value) in params {
        url.push(if first { '?' } else { '&' });
        first = false;
        url.push_str(key);
        url.push('=');
        url.push_str(&encode(value));
    }
    url
}

/// Open a things:/// URL without bringing Things to the foreground.
pub async fn open(command: &str, params: &[(&str, &str)]) -> Result<()> {
    let url = build_url(command, params);
    let output = Command::new("open")
        .arg("-g")
        .arg(&url)
        .output()
        .await
        .context("failed to spawn `open`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to open Things URL: {}", stderr.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_encodes_params() {
        let url = build_url(
            "add",
            &[
                ("title", "Buy milk & eggs"),
                ("notes", "line one\nline two"),
            ],
        );
        assert_eq!(
            url,
            "things:///add?title=Buy%20milk%20%26%20eggs&notes=line%20one%0Aline%20two"
        );
    }

    #[test]
    fn build_url_without_params() {
        assert_eq!(build_url("version", &[]), "things:///version");
    }

    #[test]
    fn build_url_encodes_url_metacharacters() {
        // Values cannot smuggle extra parameters or fragments into the URL.
        let url = build_url("update", &[("id", "abc"), ("title", "x&completed=true#f")]);
        assert_eq!(
            url,
            "things:///update?id=abc&title=x%26completed%3Dtrue%23f"
        );
    }
}

/// Auth token required by update/update-project commands.
pub fn auth_token(flag: Option<&str>) -> Result<String> {
    if let Some(token) = flag {
        return Ok(token.to_string());
    }
    match std::env::var(AUTH_TOKEN_ENV) {
        Ok(token) if !token.is_empty() => Ok(token),
        _ => bail!(
            "updating existing items requires the Things auth token. \
             Find it in Things → Settings → General → Enable Things URLs → Manage, \
             then set {AUTH_TOKEN_ENV} or pass --auth-token."
        ),
    }
}
