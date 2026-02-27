# iroh-h3

## Purpose

HTTP/3 server implementation over Iroh's QUIC transport. Enables HTTP/3 endpoints that run directly on Iroh's networking layer.

## Role in XFChess

**Currently NOT actively used in the chess program.**

This crate provides HTTP/3 server capabilities that could be used for:
- Direct peer-to-peer HTTP/3 communication
- Serverless Braid nodes
- Alternative transport to standard HTTP

## Status

| Aspect | Status |
|--------|--------|
| In workspace | ✅ Yes |
| Used by main app | ❌ No |
| Used by other crates | `iroh-h3-axum`, `iroh-h3-client` |

## Recommendation

**Keep for now** - may be used for advanced P2P features or future Braid enhancements. Consider removing if not used by other crates.

## Dependencies

- `iroh` - Core Iroh library
- `h3` - HTTP/3 protocol
- `quinn` - QUIC implementation

## Notes

- **Experimental** HTTP/3 over Iroh
- Part of advanced Iroh networking stack
- Not required for basic P2P gameplay
