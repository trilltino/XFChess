use anyhow::Context as _;
use braid_http::{client::MessageParser, BraidError};

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
            assert!(message.body.is_none());
            let patches = message.patches.context("required operation value")?;
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
        assert!(matches!(parser.finish(), Err(BraidError::BodyParse(_))));
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
    for range in ["", "[\"fen history\"]"] {
        let update = Update::patched(Version::new("path"), vec![Patch::json(range, "{}")]);
        let message = MessageParser::new()
            .feed(&format_update(&update).context("required operation value")?)
            .context("required operation value")?
            .pop()
            .context("required operation value")?;
        let patch = message
            .patches
            .context("required operation value")?
            .pop()
            .context("required operation value")?;
        assert_eq!(patch.unit, "json");
        assert_eq!(patch.range, range);
    }
    Ok(())
}
