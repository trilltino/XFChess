use h3::quic::{ConnectionErrorIncoming, StreamErrorIncoming};
use iroh::endpoint::{ConnectionError, ReadError, WriteError};
use std::sync::Arc;
pub(crate) fn internal_stream_error(message: String) -> StreamErrorIncoming {
    StreamErrorIncoming::ConnectionErrorIncoming {
        connection_error: ConnectionErrorIncoming::InternalError(message),
    }
}
pub(crate) fn convert_connection_error(e: ConnectionError) -> h3::quic::ConnectionErrorIncoming {
    match e {
        ConnectionError::ApplicationClosed(application_close) => {
            ConnectionErrorIncoming::ApplicationClose {
                error_code: application_close.error_code.into(),
            }
        }
        ConnectionError::TimedOut => ConnectionErrorIncoming::Timeout,
        error @ ConnectionError::VersionMismatch
        | error @ ConnectionError::Reset
        | error @ ConnectionError::LocallyClosed
        | error @ ConnectionError::CidsExhausted
        | error @ ConnectionError::TransportError(_)
        | error @ ConnectionError::ConnectionClosed(_) => {
            ConnectionErrorIncoming::Undefined(Arc::new(error))
        }
    }
}

pub(crate) fn convert_read_error_to_stream_error(error: ReadError) -> StreamErrorIncoming {
    match error {
        ReadError::Reset(var_int) => StreamErrorIncoming::StreamTerminated {
            error_code: var_int.into_inner(),
        },
        ReadError::ConnectionLost(connection_error) => {
            StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(connection_error),
            }
        }
        error @ ReadError::ClosedStream | error @ ReadError::ZeroRttRejected => {
            StreamErrorIncoming::Unknown(Box::new(error))
        }
    }
}

pub(crate) fn convert_write_error_to_stream_error(error: WriteError) -> StreamErrorIncoming {
    match error {
        WriteError::Stopped(var_int) => StreamErrorIncoming::StreamTerminated {
            error_code: var_int.into_inner(),
        },
        WriteError::ConnectionLost(connection_error) => {
            StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(connection_error),
            }
        }
        error @ WriteError::ClosedStream | error @ WriteError::ZeroRttRejected => {
            StreamErrorIncoming::Unknown(Box::new(error))
        }
    }
}
