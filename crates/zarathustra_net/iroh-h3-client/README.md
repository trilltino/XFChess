# iroh-h3-client

`IrohH3Client` sends HTTP/3 requests to an Iroh peer identifier in the URI authority.
Concurrent requests share connection setup and reuse an established sender. Failed
setup is evicted, and generation checks prevent an old connection from deleting a
new cache entry. Request and response bodies can stream; JSON helpers are optional.
Cached setup futures hold weak cache references. Connection drivers stop when the last
client and response body drop; an active response can outlive its client handle.

Used by the multiplayer TCP bridge. Native integration tests cover requests, payloads,
streaming, concurrent reuse, missing peers and response metadata. Unused cookie,
redirect and retry middleware has been removed; callers own retry policy.

Maintained fork of [Le-Maz/iroh-h3](https://github.com/Le-Maz/iroh-h3), MIT licensed.
