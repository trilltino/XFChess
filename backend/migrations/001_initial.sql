-- Initial database schema for XFChess backend
-- This migration creates all necessary tables for sessions, users, vault, and audit logs

-- Sessions table for game session keys
CREATE TABLE IF NOT EXISTS sessions (
    game_id   INTEGER PRIMARY KEY,
    keypair   BLOB    NOT NULL,
    wallet    TEXT    NOT NULL,
    active    INTEGER NOT NULL DEFAULT 0
);

-- Users table for authentication
CREATE TABLE IF NOT EXISTS users (
    email         TEXT PRIMARY KEY,
    password_hash TEXT NOT NULL,
    username      TEXT NOT NULL,
    wallet        TEXT
);

-- Vault table for GDPR-compliant identity storage
CREATE TABLE IF NOT EXISTS vault_users (
    pubkey TEXT PRIMARY KEY,
    blind_index_hash TEXT UNIQUE,
    encrypted_blob BLOB,
    registered_at INTEGER,
    consent_kyc BOOLEAN NOT NULL DEFAULT 0,
    consent_retention_years INTEGER NOT NULL DEFAULT 7
);

-- Audit log table for GDPR compliance (access/deletion tracking)
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pubkey TEXT NOT NULL,
    action TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);
