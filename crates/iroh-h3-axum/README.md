# iroh-h3-axum

## Purpose

Axum integration for iroh-h3. Allows building HTTP/3 servers with the Axum web framework over Iroh's QUIC transport.

## Role in XFChess

**Currently NOT actively used in the chess program.**

Provides Axum compatibility layer for `iroh-h3`:
- Axum handlers over Iroh QUIC
- Router compatibility
- Middleware support

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
- `axum` - Web framework

## Notes

- **Companion crate** to `iroh-h3`
- Only needed if building HTTP/3 Axum servers over Iroh
- Not required for current chess functionality
