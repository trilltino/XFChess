use anyhow::Context as _;

pub fn is_insecure_remote(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return true;
    };
    match url.scheme() {
        "https" => url.host_str().is_none(),
        "http" => !url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }),
        _ => true,
    }
}

pub fn prefix(text: &str, length: usize) -> &str {
    text.char_indices()
        .nth(length)
        .map_or(text, |(index, _)| &text[..index])
}

pub fn abbreviate(text: &str, length: usize) -> String {
    if text.chars().count() <= length {
        return text.to_owned();
    }
    if length < 3 {
        return prefix(text, length).to_owned();
    }
    let head = (length - 1) / 2;
    let tail = length - head - 1;
    let end = text
        .char_indices()
        .rev()
        .nth(tail - 1)
        .map_or("", |(index, _)| &text[index..]);
    format!("{}…{end}", prefix(text, head))
}

/// The header that carries an admin session (`backend::infrastructure::admin_session`).
pub const SESSION_HEADER: &str = "X-Admin-Session";

/// Signs in to `server` through `POST /admin/auth/login`, once per run, and returns the session
/// token. The name comes from `ADMIN_USERNAME` and the password from `ADMIN_PASSWORD`; whichever
/// is unset is asked for, the password without showing it.
pub fn session_token(server: &str) -> anyhow::Result<String> {
    static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    if let Some(token) = TOKEN.get() {
        return Ok(token.clone());
    }
    let username = env_or_ask("ADMIN_USERNAME", "Admin username", false)?;
    let password = env_or_ask("ADMIN_PASSWORD", "Admin password", true)?;
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")?
        .post(format!("{server}/admin/auth/login"))
        .json(&serde_json::json!({ "username": username.trim(), "password": password }))
        .send()
        .with_context(|| format!("could not reach {server}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("sign-in refused (HTTP {status}): {}", body.trim());
    }
    let session: serde_json::Value =
        serde_json::from_str(&body).context("the sign-in reply is not JSON")?;
    let token = session["token"]
        .as_str()
        .context("the sign-in reply has no token")?
        .to_string();
    Ok(TOKEN.get_or_init(|| token).clone())
}

fn env_or_ask(var: &str, label: &str, hidden: bool) -> anyhow::Result<String> {
    if let Ok(value) = std::env::var(var) {
        if !value.is_empty() {
            return Ok(value);
        }
    }
    let term = console::Term::stderr();
    term.write_str(&format!("  {label}: "))?;
    Ok(if hidden {
        term.read_secure_line()?
    } else {
        term.read_line()?
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_keys_require_tls_outside_loopback() {
        for url in [
            "https://example.com",
            "http://localhost:8090",
            "http://127.0.0.1:8090",
            "http://[::1]:8090",
        ] {
            assert!(!is_insecure_remote(url), "{url}");
        }
        for url in [
            "http://example.com",
            "http://localhost.example.com",
            "ftp://localhost",
            "invalid",
        ] {
            assert!(is_insecure_remote(url), "{url}");
        }
    }

    #[test]
    fn display_truncation_keeps_unicode_boundaries() {
        assert_eq!(prefix("棋盤 tournament", 2), "棋盤");
        assert_eq!(abbreviate("棋盤 tournament", 5), "棋盤…nt");
        assert_eq!(abbreviate("棋盤", 0), "");
    }
}
