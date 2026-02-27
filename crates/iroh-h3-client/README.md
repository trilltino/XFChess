# iroh-h3-client

## Purpose

HTTP/3 client implementation over Iroh's QUIC transport. Provides an HTTP client that can communicate over Iroh's P2P network using HTTP/3.

## Role in XFChess

**Currently NOT actively used in the chess program.**

Client-side counterpart to `iroh-h3`:
- HTTP/3 requests over Iroh QUIC
- P2P HTTP client capabilities

## Status

| Aspect | Status |
|--------|--------|
| In workspace | ✅ Yes |
| Used by main app | ❌ No |
| Depends on | `iroh-h3` |

## Recommendation

**Consider removing** unless `iroh-h3` is actively used. This is a companion crate to `iroh-h3`.

## Dependencies

- `iroh-h3` - HTTP/3 over Iroh
- `h3` - HTTP/3 protocol

## Notes

- **Companion crate** to `iroh-h3`
- Only needed for HTTP/3 client connections over Iroh
- Not required for current chess functionality
