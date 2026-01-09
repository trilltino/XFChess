-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Users Table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    elo INT NOT NULL DEFAULT 1200,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Games Table
CREATE TABLE games (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    white_player_id UUID REFERENCES users(id),
    black_player_id UUID REFERENCES users(id),
    winner_id UUID REFERENCES users(id), -- NULL if draw or ongoing
    pgn TEXT,
    start_time TIMESTAMPTZ DEFAULT NOW(),
    end_time TIMESTAMPTZ,
    status VARCHAR(20) DEFAULT 'ongoing', -- ongoing, completed, aborted
    CHECK (status IN ('ongoing', 'completed', 'aborted'))
);

-- Matchmaking Queue
CREATE TABLE matchmaking_queue (
    user_id UUID PRIMARY KEY REFERENCES users(id),
    joined_at TIMESTAMPTZ DEFAULT NOW()
);
