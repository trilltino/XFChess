# iroh-h3

QUIC adapter connecting Iroh endpoint streams to `h3`. Used by `iroh-h3-axum` and
`iroh-h3-client` in the multiplayer TCP bridge.

Receive polling uses Iroh's cancellation-safe, zero-copy `read_chunk` API. Stream
identifiers and cancellation remain available while a read is pending.

Maintained fork of [Le-Maz/iroh-h3](https://github.com/Le-Maz/iroh-h3), MIT licensed.
