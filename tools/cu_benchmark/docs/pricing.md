# Provider pricing

Checked 2026-10-10. Public list prices are planning assumptions; an existing subscription or legacy invoice can differ. `../scenarios/hetzner-triton.json` records the assumptions with each exported report.

## Hetzner

Read-only SSH inspection of the configured VPS found 2 Intel vCPUs, 4,005,572,608 bytes of usable RAM, a 40,961,572,864-byte disk and location `nbg1-dc3`. Backend, nginx and node exporter were active. This matches the CX22/CX23 class; hardware and instance metadata do not establish the purchased SKU. No Hetzner account API context was available.

The current CX23 replacement price in Germany is **€5.49 / $6.49 monthly**, excluding IPv4 and VAT. Primary IPv4 adds **€0.50 / $0.60**, giving **€5.99 / $7.09** before tax. This is a replacement-price assumption, not the VPS invoice. The June adjustment applies to new orders and rescales; legacy pricing can survive. Backups, snapshots, extra volumes and VAT need account confirmation.

EU CX instances include 20 TB of outgoing traffic. Host interface counters also contain free internal traffic and other applications, so the example treats them as a conservative estimate. Hetzner bills overage in 100 MB increments; the benchmark rounds monetary estimates upward to minor currency units rather than reproducing an invoice.

Sources: [current prices](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/), [IPv4](https://docs.hetzner.com/cloud/servers/primary-ips/overview/), [traffic](https://docs.hetzner.com/robot/general/traffic/), [billing and legacy pricing](https://docs.hetzner.com/cloud/billing/faq/).

## Triton

| Service | Current PAYG rate, USD |
| --- | --- |
| Standard RPC, unary gRPC, account and ledger queries | $10 / million calls + $0.08 / GB |
| Streaming and Titan Prime API | $0.08 / GB |
| Metaplex API | $50 / million calls + $0.08 / GB |
| Shred streaming | From $900 / month, or $0.18 / 15 minutes per IP per data centre; city dependent |
| Preconfs | Greater of $0.0015 / slot or $50 / million messages |
| Dedicated nodes | Obtain an account-specific quote |

The **$125 minimum deposit is prepaid, non-refundable credit valid for 12 months**. It is recorded under `pricing.prepaid`, separately from recurring expenses. Low usage can leave credit unused at expiry; monthly usage burn alone does not describe that cash requirement. The report does not predict credit expiry or renewals.

The current pricing page supersedes older tier descriptions for new PAYG planning. Triton's June announcement describes the move away from fixed bundles. Existing negotiated subscriptions can differ. The VPS uses a Triton **devnet** endpoint; developer subscription documentation describes devnet/testnet access as typically free. The example's PAYG rates are a paid deployment planning case, not proof that this devnet endpoint incurs those charges.

Sources: [current rates and deposit terms](https://triton.one/pricing), [June pricing announcement](https://blog.triton.one/the-end-of-the-black-box-why-rpc-pricing-needs-radical-transparency-2/), [subscription types](https://docs.triton.one/account-management/api-access/subscription-types).

## Measurement coverage

The backend's new `xfchess_rpc_http_requests_total` counts actual SDK HTTP attempts, including internal retries, and labels only provider origin, method and status. Select the Triton origin: MagicBlock and fallback traffic should not be billed at Triton's rate. HTTP attempt counts are an estimate of billable calls; reconcile errors, batches and exemptions against the provider statement.

Triton bandwidth is distinct from VPS egress. Streams deliver incoming data, while player traffic contributes VPS egress. The example references `triton_billed_bandwidth_bytes_total`, an external provider usage counter that the backend does **not** implement. Without that counter, the total remains unknown. Decimal GB is assumed until the provider confirms its billing definition. Raw HTTP body sizes cannot establish provider bandwidth charges, especially with compression, headers and streaming transports.

[Rate-limit guidance](https://docs.triton.one/core-features/ratelimits) requires an IP-wide ten-second pause after HTTP 429. SDK retry counts make that pressure visible; backend SDK backoff must be assessed separately from the benchmark's HTTP retry policy.

For illustration, 100 games producing 10,000 standard calls and 1 GB of Triton traffic would burn **$0.18** at PAYG rates. This excludes chain fees, prepaid funding and the VPS monthly bill. The number of games alone cannot establish these quantities.

Mixed currencies require `exchange_rates_millionths`, mapping each source currency to the report currency. Rates need an explicit dated assumption; missing conversion leaves the combined total unknown. Fixed VPS costs apply once per month. An allocated share of that bill is not an additional charge for each game.
