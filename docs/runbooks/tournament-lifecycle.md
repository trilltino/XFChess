# Tournament Lifecycle Runbook

Use this flow when operating or debugging tournaments.

1. Initialize the tournament, required player shards, and prize escrow.
2. Fund guaranteed prizes before paid registration opens.
3. Register players through the shard API so duplicate checks and counters stay consistent.
4. Start only when the tournament has enough players. Start sorts by ELO and initializes Swiss standings.
5. Record match results only for the stored match participants.
6. Pay prizes through `claim_tournament_prize` or `distribute_tournament_prizes`.
7. Close only after the tournament is `Completed` and every funded prize place has its claim bit set.

If a player leaves during registration, both registration counters must decrement and the player must be removed from active shard vectors.
