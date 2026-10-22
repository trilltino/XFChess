use crate::error::{BraidError, Result};
use crate::types::Version;
#[cfg(test)]
use anyhow::Context as _;

pub fn parse_version_header(value: &str) -> Result<Vec<Version>> {
    use sfv::{BareItem, List, ListEntry, Parser};
    if let Ok(list) = Parser::new(value).parse::<List>() {
        let mut versions = Vec::new();
        for member in list {
            let ListEntry::Item(item) = member else {
                return Err(BraidError::HeaderParse(
                    "Invalid version list member".into(),
                ));
            };
            versions.push(match item.bare_item {
                BareItem::String(s) => Version::String(s.into()),
                BareItem::Integer(i) => Version::Integer(i.into()),
                BareItem::Token(t) => Version::String(t.into()),
                _ => return Err(BraidError::HeaderParse("Invalid version identifier".into())),
            });
        }
        return Ok(versions);
    }

    if let Ok(json_arr) = serde_json::from_str::<Vec<String>>(value) {
        return Ok(json_arr.into_iter().map(Version::String).collect());
    }

    if let Ok(json_str) = serde_json::from_str::<String>(value) {
        return Ok(vec![Version::String(json_str)]);
    }

    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    Err(BraidError::HeaderParse("Invalid version list".into()))
}
pub fn format_version_header(versions: &[Version]) -> String {
    versions
        .iter()
        .map(|v| match v {
            Version::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Version::Integer(i) => format!("\"{}\"", i), // Force quotes around integers
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn parse_current_version_header(value: &str) -> Result<Vec<Version>> {
    parse_version_header(value)
}

pub(crate) fn parse_patch_count(
    patches: Option<&str>,
    content_type: Option<&str>,
) -> Result<Option<usize>> {
    let parse = |value: &str| {
        value
            .trim()
            .parse::<usize>()
            .map_err(|_| BraidError::HeaderParse("Invalid patch count".into()))
    };
    let explicit = patches.map(parse).transpose()?;
    let mut media_count = None;
    if let Some(content_type) = content_type {
        let mut parts = content_type.split(';');
        if parts.next().is_some_and(|media| {
            media
                .trim()
                .eq_ignore_ascii_case("application/http-patches")
        }) {
            for parameter in parts {
                if let Some((name, value)) = parameter.split_once('=') {
                    if name.trim().eq_ignore_ascii_case("count") {
                        let count = parse(value.trim().trim_matches('"'))?;
                        if media_count.replace(count).is_some() {
                            return Err(BraidError::HeaderParse("Duplicate patch count".into()));
                        }
                    }
                }
            }
        }
    }
    if explicit.zip(media_count).is_some_and(|(a, b)| a != b) {
        return Err(BraidError::HeaderParse("Conflicting patch counts".into()));
    }
    Ok(explicit.or(media_count))
}

pub fn parse_content_range(value: &str) -> Result<(String, String)> {
    let value = value.trim();
    let (unit, range) = value.split_once(char::is_whitespace).unwrap_or((value, ""));
    if unit.is_empty() {
        return Err(BraidError::HeaderParse(format!(
            "Invalid Content-Range: missing unit in '{}'",
            value
        )));
    }
    Ok((unit.to_owned(), range.trim_start().to_owned()))
}

#[inline]
pub fn format_content_range(unit: &str, range: &str) -> String {
    format!("{} {}", unit, range)
}

pub fn parse_heartbeat(value: &str) -> Result<u64> {
    let trimmed = value.trim();
    if let Some(ms_str) = trimmed.strip_suffix("ms") {
        return ms_str
            .parse::<u64>()
            .map(|n| n / 1000)
            .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {}", value)));
    }
    if let Some(s_str) = trimmed.strip_suffix('s') {
        return s_str
            .parse()
            .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {}", value)));
    }
    trimmed
        .parse()
        .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {}", value)))
}

pub fn parse_merge_type(value: &str) -> Result<String> {
    let trimmed = value.trim();
    match trimmed {
        crate::protocol::constants::merge_types::DIAMOND => Ok(trimmed.to_string()),
        _ => Err(BraidError::HeaderParse(format!(
            "Unsupported merge-type: {}",
            value
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_headers() -> anyhow::Result<()> {
        assert_eq!(
            parse_heartbeat("5s").context("required operation value")?,
            5
        );
        assert_eq!(format_content_range("json", ".f"), "json .f");
        Ok(())
    }
}
