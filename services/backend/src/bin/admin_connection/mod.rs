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
