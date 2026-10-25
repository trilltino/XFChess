#![allow(deprecated)] // The compatibility wrapper itself is under test.
use anyhow::Context as _;
use braid_http::protocol::{parse_heartbeat, parse_heartbeat_duration};
use std::time::Duration;

#[test]
#[allow(deprecated)]
fn durations_are_precise_and_legacy_seconds_round_up() -> anyhow::Result<()> {
    for (wire, duration) in [
        ("500ms", Duration::from_millis(500)),
        ("0.5", Duration::from_millis(500)),
        ("1.25s", Duration::from_millis(1250)),
        ("86400", Duration::from_secs(86400)),
    ] {
        assert_eq!(
            parse_heartbeat_duration(wire).context("unexpected failure")?,
            duration
        );
        assert_eq!(
            braid_http::client::parse_heartbeat(wire).context("unexpected failure")?,
            duration
        );
        assert_eq!(
            parse_heartbeat(wire).context("unexpected failure")?,
            duration.as_secs() + u64::from(duration.subsec_nanos() != 0)
        );
    }
    for invalid in [
        "0",
        "0ms",
        "-1",
        "NaN",
        "inf",
        "1e999",
        "86401",
        "",
        "garbage",
        "0.00000000001ms",
    ] {
        assert!(parse_heartbeat_duration(invalid).is_err(), "{invalid}");
        assert!(
            braid_http::client::HeartbeatConfig::from_header(invalid).is_none(),
            "{invalid}"
        );
    }
    Ok(())
}

#[test]
fn peer_is_one_bounded_structured_string_or_a_legacy_token() -> anyhow::Result<()> {
    use braid_http::protocol::parse_peer;
    assert_eq!(
        parse_peer("legacy-agent-7").context("unexpected failure")?,
        "legacy-agent-7"
    );
    assert_eq!(
        parse_peer(r#""quoted\"agent""#).context("unexpected failure")?,
        "quoted\"agent"
    );
    assert_eq!(
        parse_peer("%\"%c3%a9\"").context("unexpected failure")?,
        "\u{e9}"
    );
    for invalid in [
        "",
        r#""""#,
        r#""a", "b""#,
        r#""a", "a""#,
        "bad peer",
        "\"a\";x=1",
    ] {
        assert!(parse_peer(invalid).is_err(), "{invalid}");
    }
    Ok(())
}
