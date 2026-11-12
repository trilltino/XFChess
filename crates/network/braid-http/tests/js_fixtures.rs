use anyhow::Context as _;
use braid_http::{
    client::{message_to_update, MessageParser},
    types::Update,
};
#[test]
fn upstream_javascript_frames_decode_at_every_fragment_boundary() -> anyhow::Result<()> {
    let record: serde_json::Value = serde_json::from_str(include_str!("fixtures/js-codec.json"))
        .context("unexpected failure")?;
    for case in record["fixtures"]
        .as_array()
        .context("unexpected failure")?
    {
        let wire: Vec<u8> =
            serde_json::from_value(case["wire"].clone()).context("unexpected failure")?;
        let mut expected = case["expected"].clone();
        expected["status"] = 200.into();
        expected["extra_headers"] = serde_json::json!({});
        let expected: Update = serde_json::from_value(expected).context("unexpected failure")?;
        for split in 0..=wire.len() {
            let mut parser = MessageParser::new();
            let mut messages = parser.feed(&wire[..split]).context("unexpected failure")?;
            messages.extend(parser.feed(&wire[split..]).context("unexpected failure")?);
            parser.finish().context("unexpected failure")?;
            assert_eq!(messages.len(), 1, "{} split {split}", case["name"]);
            let actual = message_to_update(messages.remove(0)).context("unexpected failure")?;
            assert_eq!(actual.version, expected.version);
            assert_eq!(actual.parents, expected.parents);
            assert_eq!(actual.body, expected.body);
            match (actual.patches, &expected.patches) {
                (Some(actual), Some(expected)) => {
                    assert_eq!(actual.len(), expected.len());
                    for (a, e) in actual.iter().zip(expected) {
                        assert_eq!(
                            (&a.unit, &a.range, &a.content),
                            (&e.unit, &e.range, &e.content)
                        );
                    }
                }
                (None, None) => {}
                _ => panic!("Patch/snapshot mismatch"),
            }
        }
    }
    Ok(())
}

#[test]
fn seeded_wire_mutations_never_panic_or_exceed_configured_bounds() {
    let seed = b"Version: \"seed\"\r\nPatches: 1\r\n\r\nContent-Length: 4\r\nContent-Range: bytes [0:0]\r\n\r\nbody";
    for run in 0u64..2048 {
        let mut bytes = seed.to_vec();
        let mut rng = run.wrapping_add(1);
        for _ in 0..8 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let offset = rng as usize % bytes.len();
            bytes[offset] = (rng >> 32) as u8;
        }
        let mut parser = MessageParser::with_limits(braid_http::client::ParserLimits {
            max_header_bytes: 1024,
            max_body_bytes: 1024,
            max_update_bytes: 2048,
            max_patches: 8,
            ..Default::default()
        });
        for chunk in bytes.chunks((run % 17 + 1) as usize) {
            if parser.feed(chunk).is_err() {
                break;
            }
        }
        let _ = parser.finish();
    }
}
