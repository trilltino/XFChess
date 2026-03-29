//! Web-to-Native Session Bridge
//!
//! This module handles receiving session data from the web app via
//! environment variables or temp files. The session allows the native
//! game to sign transactions on behalf of the user's wallet using
//! an ephemeral session keypair.

pub mod session;
