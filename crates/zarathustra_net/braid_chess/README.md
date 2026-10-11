# braid_chess

Chess messages and resource paths shared by desktop and backend code.

`ChessMessage` carries moves, clocks, chat, engine analysis and session metadata.
`ChessResource` maps streams to `/game/{id}/{moves,clock,engine,chat}`.
`ChessPublisher` maintains independent parent chains for each resource;
`ChessSubscriber` decodes streamed updates into a bounded channel and exits when
its receiver drops or transport retries are exhausted. Subscriptions request heartbeats
and retry up to five consecutive failures, resuming from delivered versions.
Its returned task can also be aborted.

`subscribe_acknowledged` delivers a message and receipt. Persist its applied state before
consuming the receipt; dropping it rejects the update. `journal` stores ordered pending
writes and per-stream applied checkpoints under a backend/game/device scope. Writes use
file locking, atomic replacement and file synchronization. Session credentials remain
in memory; pending events retain their version IDs until the backend confirms them.

`TournamentResource` parses tournament document paths. `SwissMessage` converts
snapshots and supported whole-document/append JSON patches into tournament events.
Game move legality, authentication, recovery after expired history and complete tournament state
reconstruction belong to the application layer.

`version_hash` identifies a state and move number using SHA-256.
