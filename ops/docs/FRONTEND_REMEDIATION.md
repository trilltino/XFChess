# Website deployment

[deploy.ps1](../scripts/deploy.ps1) builds `xfchessdotcom` from a clean checkout
using `npm ci`, sets its backend URL for the selected TLS domain, and uploads a
complete web archive. Source and web artifacts belong to the same committed release.

The server serves `/opt/xfchess/web`, a link into the current release. Replacing
the release also switches the website, so removed build assets do not accumulate
in the served directory. Rollback restores the matching prior website.

HTTP, HTTPS and staging include one [route table](../nginx/routes.conf). Public
API routes reach the backend; Braid/SSE buffering is disabled and WebSocket
upgrades are forwarded. Bare tournament URLs remain frontend pages; use `/api`
for tournament API calls. Admin and metrics endpoints are private.

Release publication is handled by [release.yml](../../.github/workflows/release.yml)
and [scripts/release.ps1](../../scripts/release.ps1). Publication and VPS deployment
are separate operations. See the [hosted verification checklist](HOSTED_BACKEND_CHECKLIST.md).
