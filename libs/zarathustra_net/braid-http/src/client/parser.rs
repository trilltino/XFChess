use crate::error::{BraidError, Result};
use crate::types::Patch;
use bytes::{Buf, Bytes, BytesMut};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_UPDATE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PATCHES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseState {
    WaitingForHeaders,
    WaitingForBody,
    WaitingForPatchHeaders,
    WaitingForPatchBody,
    SkippingSeparator,
}

#[derive(Debug)]
pub struct MessageParser {
    buffer: BytesMut,
    state: ParseState,
    headers: BTreeMap<String, String>,
    body_buffer: BytesMut,
    expected_body_length: usize,
    read_body_length: usize,
    patches: Vec<Patch>,
    expected_patches: Option<usize>,
    patches_read: usize,
    patch_headers: BTreeMap<String, String>,
    expected_patch_length: usize,
    patch_bytes: usize,
}

static HTTP_STATUS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^HTTP/?\d*\.?\d* (\d{3})").unwrap());

static ENCODING_BLOCK_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)Encoding:\s*(\w+)\r?\nLength:\s*(\d+)\r?\n").unwrap());

impl MessageParser {
    pub fn new() -> Self {
        MessageParser {
            buffer: BytesMut::with_capacity(8192),
            state: ParseState::WaitingForHeaders,
            headers: BTreeMap::new(),
            body_buffer: BytesMut::new(),
            expected_body_length: 0,
            read_body_length: 0,
            patches: Vec::new(),
            expected_patches: None,
            patches_read: 0,
            patch_headers: BTreeMap::new(),
            expected_patch_length: 0,
            patch_bytes: 0,
        }
    }

    #[must_use]
    pub fn for_subscription() -> Self {
        MessageParser::new()
    }

    #[must_use]
    pub fn for_response(headers: BTreeMap<String, String>, content_length: usize) -> Self {
        let mut parser = MessageParser::new();
        parser.headers = headers;
        parser.expected_body_length = content_length;
        parser.state = ParseState::WaitingForBody;
        parser
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Message>> {
        if self
            .buffer
            .len()
            .saturating_add(data.len())
            .saturating_add(self.body_buffer.len())
            .saturating_add(self.patch_bytes)
            > MAX_UPDATE_BYTES
        {
            return Err(BraidError::Protocol("Update exceeds 16 MiB".into()));
        }
        self.buffer.extend_from_slice(data);
        let mut messages = Vec::new();

        loop {
            match self.state {
                ParseState::WaitingForHeaders => {
                    while !self.buffer.is_empty()
                        && (self.buffer[0] == b'\r' || self.buffer[0] == b'\n')
                    {
                        self.buffer.advance(1);
                    }

                    if self.buffer.is_empty() {
                        break;
                    }

                    if self.check_encoding_block()? {
                        self.state = ParseState::WaitingForBody;
                        continue;
                    }

                    if let Some(pos) = self.find_header_end() {
                        self.parse_headers(pos)?;
                        self.state = ParseState::WaitingForBody;
                    } else {
                        if self.buffer.len() > MAX_HEADER_BYTES {
                            return Err(BraidError::HeaderParse("Headers exceed 64 KiB".into()));
                        }
                        break;
                    }
                }
                ParseState::WaitingForBody => {
                    if let Some(n) = self.expected_patches {
                        if n == 0 {
                            // `Patches: 0` is complete at the blank line: no patch
                            // blocks and no body follow it.
                            if let Some(msg) = self.finalize_message()? {
                                messages.push(msg);
                            }
                            self.reset();
                            self.state = ParseState::WaitingForHeaders;
                        } else {
                            self.state = ParseState::WaitingForPatchHeaders;
                        }
                    } else if self.try_parse_body()? {
                        if let Some(msg) = self.finalize_message()? {
                            messages.push(msg);
                        }
                        self.reset();
                        self.state = ParseState::WaitingForHeaders;
                    } else {
                        break;
                    }
                }
                ParseState::WaitingForPatchHeaders => {
                    if let Some(pos) = self.find_header_end() {
                        self.parse_patch_headers(pos)?;
                        self.state = ParseState::WaitingForPatchBody;
                    } else {
                        if self.buffer.len() > MAX_HEADER_BYTES {
                            return Err(BraidError::HeaderParse(
                                "Patch headers exceed 64 KiB".into(),
                            ));
                        }
                        break;
                    }
                }
                ParseState::WaitingForPatchBody => {
                    if self.try_parse_patch_body()? {
                        self.patches_read += 1;
                        if self.patches_read < self.expected_patches.unwrap_or(0) {
                            self.state = ParseState::SkippingSeparator;
                        } else {
                            if let Some(msg) = self.finalize_message()? {
                                messages.push(msg);
                            }
                            self.reset();
                            self.state = ParseState::WaitingForHeaders;
                        }
                    } else {
                        break;
                    }
                }
                ParseState::SkippingSeparator => {
                    if self.buffer.len() >= 2 {
                        if &self.buffer[..2] == b"\r\n" {
                            self.buffer.advance(2);
                        } else if self.buffer[0] == b'\n' {
                            self.buffer.advance(1);
                        }
                        self.state = ParseState::WaitingForPatchHeaders;
                    } else if self.buffer.len() == 1 && self.buffer[0] == b'\n' {
                        self.buffer.advance(1);
                        self.state = ParseState::WaitingForPatchHeaders;
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(messages)
    }

    /// Checks that EOF falls between complete updates, allowing trailing heartbeats.
    pub fn finish(&self) -> Result<()> {
        if self.state == ParseState::WaitingForHeaders
            && self.buffer.iter().all(|b| matches!(b, b'\r' | b'\n'))
        {
            Ok(())
        } else {
            Err(BraidError::BodyParse(
                "Subscription ended mid-update".into(),
            ))
        }
    }

    fn check_encoding_block(&mut self) -> Result<bool> {
        if self.buffer.is_empty() || (self.buffer[0] != b'E' && self.buffer[0] != b'e') {
            return Ok(false);
        }

        if let Some(end) = self.find_double_newline() {
            let header_bytes = &self.buffer[..end];
            let header_str = std::str::from_utf8(header_bytes).map_err(|e| {
                BraidError::Protocol(format!("Invalid encoding block UTF-8: {}", e))
            })?;

            if let Some(caps) = ENCODING_BLOCK_REGEX.captures(header_str) {
                let encoding = caps.get(1).unwrap().as_str().to_string();
                let length: usize = caps.get(2).unwrap().as_str().parse().map_err(|_| {
                    BraidError::Protocol("Invalid length in encoding block".to_string())
                })?;

                let _ = self.buffer.split_to(end);
                self.headers.insert("encoding".to_string(), encoding);
                self.headers
                    .insert("length".to_string(), length.to_string());
                self.expected_body_length = length;
                if length > MAX_UPDATE_BYTES {
                    return Err(BraidError::Protocol("Encoding block exceeds 16 MiB".into()));
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn find_double_newline(&self) -> Option<usize> {
        if let Some(pos) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            return Some(pos + 4);
        }
        if let Some(pos) = self.buffer.windows(2).position(|w| w == b"\n\n") {
            return Some(pos + 2);
        }
        None
    }

    fn find_header_end(&self) -> Option<usize> {
        self.buffer.iter().enumerate().find_map(|(i, b)| {
            if *b != b'\n' {
                return None;
            }
            if self.buffer.get(i + 1) == Some(&b'\n') {
                Some(i + 2)
            } else if self.buffer.get(i + 1..i + 3) == Some(b"\r\n") {
                Some(i + 3)
            } else {
                None
            }
        })
    }

    fn parse_headers(&mut self, end: usize) -> Result<()> {
        if end > MAX_HEADER_BYTES {
            return Err(BraidError::HeaderParse("Headers exceed 64 KiB".into()));
        }
        let header_bytes = self.buffer.split_to(end);
        let mut header_str = String::from_utf8(header_bytes.to_vec())?;

        if let Some(caps) = HTTP_STATUS_REGEX.captures(&header_str) {
            if let Some(status_match) = caps.get(1) {
                let status = status_match.as_str();
                if let Some(first_newline) = header_str.find('\n') {
                    let replacement = format!(":status: {}\r", status);
                    header_str = replacement + &header_str[first_newline..];
                }
            }
        }

        for line in header_str.lines() {
            if line.is_empty() {
                continue;
            }
            if let Some(colon_pos) = line[usize::from(line.starts_with(':'))..]
                .find(':')
                .map(|pos| pos + usize::from(line.starts_with(':')))
            {
                let key = line[..colon_pos].trim().to_lowercase();
                let value = line[colon_pos + 1..].trim().to_string();
                let duplicate = self.headers.insert(key.clone(), value).is_some();
                if duplicate && matches!(key.as_str(), "content-length" | "patches" | "length") {
                    return Err(BraidError::HeaderParse("Duplicate framing header".into()));
                }
            } else {
                return Err(BraidError::HeaderParse("Malformed update header".into()));
            }
        }

        self.expected_patches = crate::protocol::headers::parse_patch_count(
            self.headers.get("patches").map(String::as_str),
            self.headers.get("content-type").map(String::as_str),
        )?;
        if self
            .expected_patches
            .is_some_and(|count| count > MAX_PATCHES)
        {
            return Err(BraidError::Protocol("Too many patches".into()));
        }

        if let Some(len_str) = self
            .headers
            .get("content-length")
            .or_else(|| self.headers.get("length"))
        {
            self.expected_body_length = len_str.parse().map_err(|_| {
                BraidError::HeaderParse(format!("Invalid content-length: {}", len_str))
            })?;
        } else if self.expected_patches.is_none() {
            return Err(BraidError::Protocol(
                "Snapshot requires Content-Length".into(),
            ));
        }
        if self.expected_body_length > MAX_UPDATE_BYTES {
            return Err(BraidError::Protocol("Body exceeds 16 MiB".into()));
        }
        Ok(())
    }

    fn parse_patch_headers(&mut self, end: usize) -> Result<()> {
        if end > MAX_HEADER_BYTES {
            return Err(BraidError::HeaderParse(
                "Patch headers exceed 64 KiB".into(),
            ));
        }
        let header_bytes = self.buffer.split_to(end);
        let header_str = String::from_utf8(header_bytes.to_vec())?;

        self.patch_headers.clear();
        for line in header_str.lines() {
            if line.is_empty() {
                continue;
            }
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_lowercase();
                let value = line[colon_pos + 1..].trim().to_string();
                let duplicate = self.patch_headers.insert(key.clone(), value).is_some();
                if duplicate && key == "content-length" {
                    return Err(BraidError::HeaderParse("Duplicate patch length".into()));
                }
            } else {
                return Err(BraidError::HeaderParse("Malformed patch header".into()));
            }
        }

        if let Some(len_str) = self.patch_headers.get("content-length") {
            self.expected_patch_length = len_str.parse().map_err(|_| {
                BraidError::HeaderParse(format!("Invalid patch content-length: {}", len_str))
            })?;
        } else {
            return Err(BraidError::Protocol(
                "Every patch MUST include Content-Length".to_string(),
            ));
        }
        if self.expected_patch_length.saturating_add(self.patch_bytes) > MAX_UPDATE_BYTES {
            return Err(BraidError::Protocol("Patches exceed 16 MiB".into()));
        }
        if !self.patch_headers.contains_key("content-range") {
            return Err(BraidError::HeaderParse(
                "Patch requires Content-Range".into(),
            ));
        }
        Ok(())
    }

    fn try_parse_patch_body(&mut self) -> Result<bool> {
        let remaining = self.expected_patch_length;
        if self.buffer.len() >= remaining {
            let body_chunk = self.buffer.split_to(remaining);
            let content_range = self
                .patch_headers
                .get("content-range")
                .ok_or_else(|| BraidError::HeaderParse("Patch requires Content-Range".into()))?;
            let (unit, range) = crate::protocol::parse_content_range(content_range)?;
            let patch = Patch::with_length(unit, range, body_chunk, self.expected_patch_length);
            self.patches.push(patch);
            self.patch_bytes += remaining;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn try_parse_body(&mut self) -> Result<bool> {
        // For non-chunked with known content-length
        if self.expected_body_length == 0 {
            // No body expected (e.g., HEAD request or empty response with explicit 0 length)
            return Ok(true);
        }

        let remaining = self.expected_body_length - self.read_body_length;
        if self.buffer.len() >= remaining {
            let body_chunk = self.buffer.split_to(remaining);
            if self.body_buffer.is_empty() {
                self.body_buffer = body_chunk;
            } else {
                self.body_buffer.extend_from_slice(&body_chunk);
            }
            self.read_body_length += remaining;
            Ok(true)
        } else {
            let chunk_len = self.buffer.len();
            self.body_buffer
                .extend_from_slice(&self.buffer.split_to(chunk_len));
            self.read_body_length += chunk_len;
            Ok(false)
        }
    }

    fn finalize_message(&mut self) -> Result<Option<Message>> {
        let headers = std::mem::take(&mut self.headers);
        let url = headers.get("content-location").cloned();
        let encoding = headers.get("encoding").cloned();

        // An explicit zero patch count is an empty patch update, not a snapshot.
        let (body, patches) = if self.expected_patches.is_some() {
            self.body_buffer.clear();
            (None, Some(std::mem::take(&mut self.patches)))
        } else if let Some(range) = headers.get("content-range") {
            let (unit, range) = crate::protocol::parse_content_range(range)?;
            (
                None,
                Some(vec![Patch::new(
                    unit,
                    range,
                    self.body_buffer.split().freeze(),
                )]),
            )
        } else {
            (Some(self.body_buffer.split().freeze()), None)
        };

        Ok(Some(Message {
            headers,
            body,
            patches,
            status_code: None,
            encoding,
            url,
        }))
    }

    fn reset(&mut self) {
        self.headers.clear();
        self.body_buffer.clear();
        self.expected_body_length = 0;
        self.read_body_length = 0;
        self.patches.clear();
        self.expected_patches = None;
        self.patches_read = 0;
        self.patch_headers.clear();
        self.expected_patch_length = 0;
        self.patch_bytes = 0;
    }

    pub fn state(&self) -> ParseState {
        self.state
    }
    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }
    pub fn body(&self) -> &[u8] {
        &self.body_buffer
    }
}

impl Default for MessageParser {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub headers: BTreeMap<String, String>,
    pub body: Option<Bytes>,
    pub patches: Option<Vec<Patch>>,
    pub status_code: Option<u16>,
    pub encoding: Option<String>,
    pub url: Option<String>,
}

impl Message {
    pub fn status(&self) -> Option<u16> {
        self.status_code
            .or_else(|| self.headers.get(":status").and_then(|v| v.parse().ok()))
    }

    pub fn version(&self) -> Option<&str> {
        self.headers.get("version").map(|s| s.as_str())
    }
    pub fn current_version(&self) -> Option<&str> {
        self.headers.get("current-version").map(|s| s.as_str())
    }
    pub fn parents(&self) -> Option<&str> {
        self.headers.get("parents").map(|s| s.as_str())
    }

    pub fn decode_body(&self) -> Result<Bytes> {
        let body = self.body.clone().unwrap_or_default();
        match self.encoding.as_deref() {
            Some("dt") | None => Ok(body),
            Some(enc) => Err(BraidError::Protocol(format!("Unknown encoding: {}", enc))),
        }
    }

    pub fn extra_headers(&self) -> BTreeMap<String, String> {
        const KNOWN_HEADERS: &[&str] = &[
            "version",
            "parents",
            "current-version",
            "patches",
            "content-length",
            "content-range",
            ":status",
        ];
        self.headers
            .iter()
            .filter(|(k, _)| !KNOWN_HEADERS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn body_text(&self) -> Option<String> {
        let body = self.body.as_ref()?;
        std::str::from_utf8(body).ok().map(ToString::to_string)
    }
}

pub fn parse_status_line(line: &str) -> Option<u16> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0].to_uppercase().starts_with("HTTP") {
        parts[1].parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_and_malformed_updates_are_rejected_before_allocating_bodies() {
        for wire in [
            b"Content-Length: 999999999\r\n\r\n".as_slice(),
            b"Patches: 99999\r\n\r\n".as_slice(),
            b"Patches: 1\r\n\r\nContent-Length: 999999999\r\nContent-Range: bytes 0:1\r\n\r\n"
                .as_slice(),
            b"Patches: 1\r\n\r\nContent-Length: 1\r\n\r\nx".as_slice(),
        ] {
            assert!(MessageParser::new().feed(wire).is_err());
        }
        assert!(MessageParser::new()
            .feed(&vec![b'a'; MAX_HEADER_BYTES + 1])
            .is_err());
    }

    #[test]
    fn test_parser_creation() {
        let parser = MessageParser::new();
        assert_eq!(parser.state(), ParseState::WaitingForHeaders);
    }

    #[test]
    fn test_simple_message_parsing() {
        let mut parser = MessageParser::new();
        let data = b"Content-Length: 5\r\n\r\nHello";
        let messages = parser.feed(data).unwrap();
        assert!(!messages.is_empty());
        assert_eq!(messages[0].body, Some(Bytes::from_static(b"Hello")));
        assert!(
            messages[0].patches.is_none(),
            "a body is not a patch update"
        );
    }

    #[test]
    fn zero_patches_is_a_patch_update_not_a_snapshot() {
        // The distinction `braid-fuzz` tests: `Patches: 0` carries no body, and
        // must not be mistaken for a snapshot whose body happens to be empty.
        let mut parser = MessageParser::new();
        let messages = parser
            .feed(b"Version: \"v1\"\r\nPatches: 0\r\n\r\n")
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].patches, Some(vec![]));
        assert!(messages[0].body.is_none());
    }

    #[test]
    fn an_empty_snapshot_is_not_a_patch_update() {
        let mut parser = MessageParser::new();
        let messages = parser
            .feed(b"Version: \"v1\"\r\nContent-Length: 0\r\n\r\n")
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, Some(Bytes::new()));
        assert!(messages[0].patches.is_none());
    }

    #[test]
    fn a_heartbeat_yields_no_messages() {
        // A bare CRLF must be absorbed. If it ever decodes to a Message again,
        // every heartbeat becomes a versionless update and floods the logs.
        let mut parser = MessageParser::for_subscription();
        assert!(parser.feed(b"\r\n").unwrap().is_empty());
        assert!(parser.feed(b"\r\n\r\n\r\n").unwrap().is_empty());
    }

    #[test]
    fn a_bad_patches_count_is_an_error_not_a_silent_zero() {
        let mut parser = MessageParser::new();
        assert!(parser.feed(b"Patches: not-a-number\r\n\r\n").is_err());
    }

    #[test]
    fn test_parse_status_line() {
        assert_eq!(parse_status_line("HTTP/1.1 200 OK"), Some(200));
        assert_eq!(parse_status_line("HTTP 209 Subscription"), Some(209));
        assert_eq!(parse_status_line("HTTP/2 404"), Some(404));
    }

    #[test]
    fn test_message_extra_headers() {
        let mut headers = BTreeMap::new();
        headers.insert("version".to_string(), "\"v1\"".to_string());
        headers.insert("x-custom-header".to_string(), "value".to_string());

        let msg = Message {
            headers,
            body: Some(Bytes::new()),
            patches: None,
            status_code: None,
            encoding: None,
            url: None,
        };

        let extra = msg.extra_headers();
        assert_eq!(extra.len(), 1);
        assert!(extra.contains_key("x-custom-header"));
        assert!(!extra.contains_key("version"));
    }

    #[test]
    fn test_multi_patch_parsing() {
        let mut parser = MessageParser::new();
        let data = b"Patches: 2\r\n\r\n\
                     Content-Length: 5\r\n\
                     Content-Range: json .a\r\n\r\n\
                     hello\r\n\
                     Content-Length: 5\r\n\
                     Content-Range: json .b\r\n\r\n\
                     world\r\n";

        let messages = parser.feed(data).unwrap();
        assert!(!messages.is_empty());
        let patches = messages[0].patches.as_ref().expect("a patch update");
        assert_eq!(patches.len(), 2);
        assert_eq!(patches[0].range, ".a");
        assert_eq!(patches[1].range, ".b");
        assert!(messages[0].body.is_none());
    }
}
