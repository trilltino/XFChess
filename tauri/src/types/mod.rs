//! Unused DTO scaffolding — `auth`, `config`, and `ipc` here are all
//! confirmed dead (each carries its own `#![allow(dead_code)]`). The bridge's
//! real config lives directly in `main.rs` / `services::config`, and its real
//! IPC command set is `services::ipc::{WindowCommands, IpcCommands}` —
//! similarly named but distinct from, and not backed by, the types in this
//! module. See each submodule's docs for specifics.

pub mod auth;
pub mod config;
pub mod ipc;
