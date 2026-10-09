CREATE TABLE IF NOT EXISTS casual_game_participants (
    game_id TEXT PRIMARY KEY,
    host_node_id TEXT NOT NULL,
    joiner_node_id TEXT NOT NULL
);
