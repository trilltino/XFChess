# Client crate architecture

The executable package at `apps/xfchess-client` is intentionally a thin host.
Its desktop and Android entry points call into the client crates through this
one-way dependency graph:

```text
xfchess-platform-api       xfchess-client-domain
          │                         │
          └──────────┬──────────────┘
                     ▼
          xfchess-client-network
                     │
                     ▼
          xfchess-client-solana
                     │
                     ▼
           xfchess-client-bevy
                     │
                     ▼
               host package
```

- `xfchess-platform-api` contains host capability and wallet request contracts.
- `xfchess-client-domain` contains renderer- and transport-free client rules and types.
- `xfchess-client-network` owns backend authentication, wire messages, identity, and ordering.
- `xfchess-client-solana` owns instruction construction and wallet-neutral signing contracts.
- `xfchess-client-bevy` composes ECS systems, UI, rendering, input, and platform implementations.

Lower layers must not import Bevy or the host package. Platform implementations
are registered by `xfchess-client-bevy`; the API crate contains contracts only.
