use anyhow::Context as _;
use braid_http::client::MessageParser;

#[test]
fn upstream_wire_forms_parse_across_every_chunk_boundary() -> anyhow::Result<()> {
    for wire in [
        b"HTTP 200 OK\nVersion: \"v2\"\nParents: \"v1\"\nContent-Length: 2\nContent-Range: bytes [0:1]\n\n\xff\xfe\n".as_slice(),
        b"Version: \"v2\"\r\nContent-Type: application/http-patches; count=1\r\n\r\nContent-Length: 2\r\nContent-Range: bytes [0:1]\r\n\r\n\xff\xfe\r\n",
    ] {
        for boundary in 0..=wire.len() {
            let mut parser = MessageParser::new();
            let mut messages = parser.feed(&wire[..boundary]).context("required operation value")?;
            messages.extend(parser.feed(&wire[boundary..]).context("required operation value")?);
            parser.finish().context("required operation value")?;
            assert_eq!(messages.len(), 1, "split at {boundary}");
            let message = messages.pop().context("required operation value")?;
            let patches = braid_http::client::message_to_update(message)?
                .patches
                .context("a range frame is a patched update")?;
            assert_eq!(patches.len(), 1);
            assert_eq!(patches[0].unit, "bytes");
            assert_eq!(patches[0].range, "[0:1]");
            assert_eq!(patches[0].content.as_ref(), &[255, 254]);
        }
    }
    Ok(())
}

#[test]
fn premature_eof_and_ambiguous_framing_are_errors() -> anyhow::Result<()> {
    for wire in [
        b"Version: \"v".as_slice(),
        b"Content-Length: 3\r\n\r\nab",
        b"Patches: 1\r\n\r\nContent-Length: 2\r\nContent-Range: bytes [0:1]\r\n\r\na",
    ] {
        let mut parser = MessageParser::new();
        parser.feed(wire).context("required operation value")?;
        assert!(parser.finish().is_err(), "truncated input must fail closed");
    }
    for wire in [
        b"Content-Length: 1\nContent-Length: 2\n\nab".as_slice(),
        b"Patches: 1\nContent-Type: application/http-patches; count=2\n\n",
        b"Content-Type: application/http-patches; count=nope\n\n",
        b"not a header\nContent-Length: 0\n\n",
    ] {
        assert!(MessageParser::new().feed(wire).is_err());
    }
    Ok(())
}

#[test]
fn multiresponse_status_is_preserved() -> anyhow::Result<()> {
    let message = MessageParser::new()
        .feed(b"HTTP 410 Gone\r\nContent-Length: 0\r\n\r\n")
        .context("required operation value")?
        .pop()
        .context("required operation value")?;
    assert_eq!(message.status(), Some(410));
    Ok(())
}

#[test]
fn root_ranges_and_quoted_paths_preserve_the_complete_expression() -> anyhow::Result<()> {
    use braid_http::{protocol::format_update, Patch, Update, Version};
    // A root replacement is a snapshot, so an empty range is refused when formatting.
    let rootless = Update::patched(Version::new("path"), vec![Patch::json("", "{}")]);
    assert!(format_update(&rootless).is_err());
    let range = "[\"fen history\"]";
    let update = Update::patched(Version::new("path"), vec![Patch::json(range, "{}")]);
    let mut message = MessageParser::new()
        .feed(&format_update(&update).context("required operation value")?)
        .context("required operation value")?
        .pop()
        .context("required operation value")?;
    let patch = message.patches.pop().context("required operation value")?;
    assert_eq!(patch.unit, "json");
    assert_eq!(patch.range, range);
    Ok(())
}

#[test]
fn heartbeats_between_frames_never_split_or_swallow_updates() -> anyhow::Result<()> {
    use braid_http::{
        client::message_to_update,
        protocol::{format_heartbeat, format_update},
        Update, Version,
    };
    let first = format_update(&Update::snapshot(Version::new("1"), "a"))?;
    let second = format_update(&Update::snapshot(Version::new("2"), "b"))?;
    let wire = [
        format_heartbeat().to_vec(),
        first.to_vec(),
        format_heartbeat().to_vec(),
        format_heartbeat().to_vec(),
        second.to_vec(),
        format_heartbeat().to_vec(),
    ]
    .concat();
    for split in 0..=wire.len() {
        let mut parser = MessageParser::for_subscription();
        let mut messages = parser.feed(&wire[..split])?;
        messages.extend(parser.feed(&wire[split..])?);
        parser.finish()?;
        let bodies = messages
            .into_iter()
            .map(|message| Ok(message_to_update(message)?.body_str().map(str::to_owned)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        assert_eq!(
            bodies,
            [Some("a".to_owned()), Some("b".to_owned())],
            "split {split}"
        );
    }
    Ok(())
}

#[test]
fn version_headers_written_by_earlier_xfchess_builds_still_parse() -> anyhow::Result<()> {
    use braid_http::protocol::{format_version_header, parse_version_header};
    use braid_http::Version;
    // Earlier builds always wrote quoted strings, including for numeric
    // backend sequence numbers, so the stricter Versions-05 grammar accepts them.
    assert_eq!(parse_version_header("\"7\"")?, [Version::new("7")]);
    assert_eq!(
        parse_version_header("\"3\", \"7\"")?,
        [Version::new("3"), Version::new("7")]
    );
    assert_eq!(format_version_header(&[Version::Integer(12)]), "\"12\"");
    // Forms that only foreign producers used are rejected deliberately rather than
    // reinterpreted: an ambiguous cursor must not silently resume from the wrong place.
    for rejected in ["7", "bare-token", "[\"a\"]"] {
        assert!(parse_version_header(rejected).is_err(), "{rejected}");
    }
    Ok(())
}
