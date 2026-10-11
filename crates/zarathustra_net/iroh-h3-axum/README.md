# iroh-h3-axum

`IrohAxum` serves an Axum router through the Iroh HTTP/3 adapter. Requests include
`RemoteId`, the authenticated Iroh peer identifier. Response bodies stream without
buffering; a body error resets the stream instead of finishing a truncated response.

Used by `braid-iroh` for resource reads, writes and HTTP 209 subscriptions.
Native integration tests exercise peer identity extraction.

Maintained fork of [Le-Maz/iroh-h3](https://github.com/Le-Maz/iroh-h3), MIT licensed.
