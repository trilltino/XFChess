# docs/

Operational and architectural documentation. Component-level docs live next to the
code (each directory's README); this tree holds the cross-cutting material.

## Index

| Path | Contents |
|------|----------|
| [adr/](adr/) | Architecture decision records (settlement split, magic-router routing, canonical settlement, tournament shards, profile init) |
| [architecture/](architecture/) | Deep dives: [magicblock-game-lifecycle.md](architecture/magicblock-game-lifecycle.md), [xfchess-game-crate.md](architecture/xfchess-game-crate.md) |
| [runbooks/](runbooks/README.md) | Incident runbooks (backend down, settlement stuck, RPC degraded, …) |
| [plans/](plans/) | Active implementation plans |
| [legacy-cleanup-audit.md](legacy-cleanup-audit.md) | Open list of stale modules/bins to remove |
| [THREAT_MODEL.md](THREAT_MODEL.md), [SLO.md](SLO.md), [CAPACITY.md](CAPACITY.md), [SCALING.md](SCALING.md), [DR.md](DR.md) | Production posture: threats, SLOs, capacity, scaling, disaster recovery |
| [ENVIRONMENTS.md](ENVIRONMENTS.md) | Environment matrix (local / staging / prod) |
| [GIT_WORKFLOW.md](GIT_WORKFLOW.md) | Branch and commit conventions |
| [PUBLISHING.md](PUBLISHING.md) | Cutting a Win/Mac/Linux release; known CI/release landmines and how they were fixed |
| [PRODUCTION_REALITY_PLAN.md](PRODUCTION_REALITY_PLAN.md) | Production-hardening master plan |

Deployment docs live in [deploy/](../deploy/README.md); the MagicBlock ER guide is
[MAGICBLOCK.md](../MAGICBLOCK.md) at the repo root.
