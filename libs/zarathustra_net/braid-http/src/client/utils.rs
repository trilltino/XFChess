use crate::client::parser::Message;
use crate::error::{BraidError, Result};
use crate::protocol;
use crate::types::{ContentRange, Update};
#[cfg(test)]
use anyhow::Context as _;
use std::time::Duration;

pub fn parse_content_range(header: &str) -> Result<(String, String)> {
    protocol::parse_content_range(header)
}
pub fn format_content_range(unit: &str, range: &str) -> String {
    protocol::format_content_range(unit, range)
}

pub fn parse_heartbeat(value: &str) -> Result<Duration> {
    let value = value.trim();
    let (number, scale) = if let Some(n) = value.strip_suffix("ms") {
        (n, 0.001)
    } else {
        (value.strip_suffix('s').unwrap_or(value), 1.0)
    };
    let seconds: f64 = number
        .parse()
        .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {value}")))?;
    Duration::try_from_secs_f64(seconds * scale)
        .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {value}")))
}

pub fn message_to_update(msg: Message) -> Result<Update> {
    let status = msg.status().unwrap_or(209);
    let version = msg
        .headers
        .get("version")
        .map(|v| protocol::parse_version_header(v))
        .transpose()?
        .unwrap_or_default();
    let parents = msg
        .headers
        .get("parents")
        .map(|v| protocol::parse_version_header(v))
        .transpose()?
        .unwrap_or_default();
    let current_version = msg
        .headers
        .get("current-version")
        .map(|v| protocol::parse_version_header(v))
        .transpose()?;
    let content_range = msg
        .headers
        .get("content-range")
        .map(|v| {
            protocol::parse_content_range(v).map(|(unit, range)| ContentRange::new(unit, range))
        })
        .transpose()?;
    Ok(Update {
        version,
        parents,
        current_version,
        content_range,
        merge_type: msg.headers.get("merge-type").cloned(),
        content_type: msg.headers.get("content-type").cloned(),
        body: msg.body,
        patches: msg.patches,
        status,
        extra_headers: msg
            .headers
            .into_iter()
            .filter(|(name, _)| {
                !matches!(
                    name.as_str(),
                    "version"
                        | ":status"
                        | "parents"
                        | "current-version"
                        | "content-range"
                        | "merge-type"
                        | "content-type"
                        | "content-length"
                        | "patches"
                )
            })
            .collect(),
        url: msg.url,
    })
}

pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::MessageParser;
    #[test]
    fn conversion_preserves_binary_bodies_versions_and_patch_metadata() -> anyhow::Result<()> {
        let mut parser = MessageParser::for_subscription();
        let message = parser.feed(b"Version: \"a\", \"b\"\r\nParents: \"old\"\r\nContent-Type: application/json-patch+json\r\nContent-Length: 2\r\n\r\n\xff\xfe").context("required operation value")?.pop().context("required operation value")?;
        let update = message_to_update(message).context("required operation value")?;
        assert_eq!(update.version.len(), 2);
        assert_eq!(update.parents[0].to_string(), "old");
        assert_eq!(
            update.content_type.as_deref(),
            Some("application/json-patch+json")
        );
        assert_eq!(
            update.body.context("required operation value")?.as_ref(),
            &[255, 254]
        );
        Ok(())
    }
    #[test]
    fn heartbeat_units_and_invalid_values() -> anyhow::Result<()> {
        assert_eq!(
            parse_heartbeat("20ms").context("required operation value")?,
            Duration::from_millis(20)
        );
        for value in ["-1", "NaN", "inf", "1e300"] {
            assert!(parse_heartbeat(value).is_err());
        }
        Ok(())
    }
}
