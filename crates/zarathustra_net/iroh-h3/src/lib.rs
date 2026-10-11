//! HTTP/3 transport adapter for Iroh QUIC connections.
#![deny(missing_docs)]

mod connection;
mod error;
mod streams;

pub use connection::{Connection, OpenStreams};
pub use iroh::endpoint::{AcceptBi, AcceptUni, Endpoint, OpenBi, OpenUni, VarInt};
pub use streams::{BidiStream, RecvStream, SendStream};
