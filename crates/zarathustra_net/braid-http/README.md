# braid-http

Shared Braid-HTTP types, wire codec and native HTTP client.

- `types`: versions, snapshots, patches and request/response metadata.
- `protocol`: header parsing and update formatting.
- `client`: incremental parsing, request execution, bounded delivery and retry policy.
- `server`: subscription response headers and request helpers.

HTTP subscriptions require status 209. The handshake uses the configured request
timeout; active subscriptions have no total lifetime limit. Advertised heartbeats
reset the wire idle deadline even when they produce no updates. Dropping a subscription
closes its delivery channel and cancels transport, backoff and reconnection work.
`BraidRequest::with_retry` enables reconnects for EOF, transport errors and idle timeouts,
including transient handshake failures and `Retry-After`. Authentication and parse errors
are terminal. The retry budget resets after a complete update is delivered.

By default, reconnect requests advance `Parents` when updates enter the delivery queue.
`with_manual_ack` instead waits for `Subscription::acknowledger`: persist application state,
then acknowledge the update's versions. Rejecting terminates the subscription without
advancing its cursor; resubscribe after recovery. Delivery is ordered with one unacknowledged
update at a time. Without explicit retry configuration, streams end on closure.
Native HTTPS requests can negotiate HTTP/2 and reuse its streams.

The parser accepts CRLF/LF framing, snapshots, single range patches and patch blocks
whose count comes from `Patches` or `Content-Type`. Truncated EOF and conflicting framing
are errors. Limits are 64 KiB for headers,
16 MiB for an update and 1,024 patches. Payloads remain binary, and all version heads,
parents, content types and custom metadata survive conversion into `Update`.

`ClientConfig` controls request/connect timeouts, proxy configuration, idle pooling and
finite request retries. The `fuzzing` feature provides arbitrary protocol types.
The implementation supports native platforms. The former browser backend was a stub
and has been removed.

To test against the upstream JavaScript implementation, clone
[braid-org/braid-http](https://github.com/braid-org/braid-http), set `BRAID_JS_SOURCE`
to that checkout and run `cargo test -p braid-http --test javascript -- --ignored`.
This optional test requires Node.js; ordinary tests have no JavaScript dependency.
