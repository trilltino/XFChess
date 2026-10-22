use crate::{
    AcceptBi, AcceptUni, OpenBi, OpenUni, VarInt,
    error::convert_connection_error,
    streams::{BidiStream, RecvStream, SendStream},
};
use bytes::Buf;
use futures::{Stream, StreamExt, ready, stream};
use h3::{
    error::Code,
    quic::{self, ConnectionErrorIncoming, StreamErrorIncoming},
};
use std::{
    future::Future,
    pin::Pin,
    task::{self, Poll},
};
type BoxStreamSync<'a, T> = Pin<Box<dyn Stream<Item = T> + Sync + Send + 'a>>;

/// An h3 QUIC connection backed by an Iroh endpoint connection.
pub struct Connection {
    conn: iroh::endpoint::Connection,
    incoming_bi: BoxStreamSync<'static, <AcceptBi<'static> as Future>::Output>,
    opening_bi: Option<BoxStreamSync<'static, <OpenBi<'static> as Future>::Output>>,
    incoming_uni: BoxStreamSync<'static, <AcceptUni<'static> as Future>::Output>,
    opening_uni: Option<BoxStreamSync<'static, <OpenUni<'static> as Future>::Output>>,
}

impl Connection {
    /// Creates a new [`Connection`] wrapping the given Iroh endpoint connection.
    pub fn new(conn: iroh::endpoint::Connection) -> Self {
        Self {
            conn: conn.clone(),
            incoming_bi: Box::pin(stream::unfold(conn.clone(), |conn| async {
                Some((conn.accept_bi().await, conn))
            })),
            opening_bi: None,
            incoming_uni: Box::pin(stream::unfold(conn, |conn| async {
                Some((conn.accept_uni().await, conn))
            })),
            opening_uni: None,
        }
    }
}

impl<B> quic::Connection<B> for Connection
where
    B: Buf,
{
    type RecvStream = RecvStream;
    type OpenStreams = OpenStreams;

    fn poll_accept_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::BidiStream, ConnectionErrorIncoming>> {
        let (send, recv) = ready!(self.incoming_bi.poll_next_unpin(cx))
            .ok_or_else(|| {
                ConnectionErrorIncoming::InternalError(
                    "incoming bidirectional streams ended".into(),
                )
            })?
            .map_err(convert_connection_error)?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send).map_err(ConnectionErrorIncoming::InternalError)?,
            recv: Self::RecvStream::new(recv).map_err(ConnectionErrorIncoming::InternalError)?,
        }))
    }

    fn poll_accept_recv(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::RecvStream, ConnectionErrorIncoming>> {
        let recv = ready!(self.incoming_uni.poll_next_unpin(cx))
            .ok_or_else(|| {
                ConnectionErrorIncoming::InternalError(
                    "incoming unidirectional streams ended".into(),
                )
            })?
            .map_err(convert_connection_error)?;
        Poll::Ready(Self::RecvStream::new(recv).map_err(ConnectionErrorIncoming::InternalError))
    }

    fn opener(&self) -> Self::OpenStreams {
        OpenStreams {
            conn: self.conn.clone(),
            opening_bi: None,
            opening_uni: None,
        }
    }
}

impl<B> quic::OpenStreams<B> for Connection
where
    B: Buf,
{
    type SendStream = SendStream<B>;
    type BidiStream = BidiStream<B>;

    fn poll_open_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::BidiStream, StreamErrorIncoming>> {
        let bi = self.opening_bi.get_or_insert_with(|| {
            Box::pin(stream::unfold(self.conn.clone(), |conn| async {
                Some((conn.open_bi().await, conn))
            }))
        });
        let (send, recv) = ready!(bi.poll_next_unpin(cx))
            .ok_or_else(|| {
                crate::error::internal_stream_error("outgoing bidirectional streams ended".into())
            })?
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send).map_err(crate::error::internal_stream_error)?,
            recv: RecvStream::new(recv).map_err(crate::error::internal_stream_error)?,
        }))
    }

    fn poll_open_send(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::SendStream, StreamErrorIncoming>> {
        let uni = self.opening_uni.get_or_insert_with(|| {
            Box::pin(stream::unfold(self.conn.clone(), |conn| async {
                Some((conn.open_uni().await, conn))
            }))
        });

        let send = ready!(uni.poll_next_unpin(cx))
            .ok_or_else(|| {
                crate::error::internal_stream_error("outgoing unidirectional streams ended".into())
            })?
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Self::SendStream::new(send).map_err(crate::error::internal_stream_error))
    }

    fn close(&mut self, code: Code, reason: &[u8]) {
        close_connection(&self.conn, code, reason);
    }
}

/// A handle for opening new outgoing streams on an Iroh QUIC connection.
pub struct OpenStreams {
    conn: iroh::endpoint::Connection,
    opening_bi: Option<BoxStreamSync<'static, <OpenBi<'static> as Future>::Output>>,
    opening_uni: Option<BoxStreamSync<'static, <OpenUni<'static> as Future>::Output>>,
}

impl<B> quic::OpenStreams<B> for OpenStreams
where
    B: Buf,
{
    type SendStream = SendStream<B>;
    type BidiStream = BidiStream<B>;

    fn poll_open_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::BidiStream, StreamErrorIncoming>> {
        let bi = self.opening_bi.get_or_insert_with(|| {
            Box::pin(stream::unfold(self.conn.clone(), |conn| async {
                Some((conn.open_bi().await, conn))
            }))
        });

        let (send, recv) = ready!(bi.poll_next_unpin(cx))
            .ok_or_else(|| {
                crate::error::internal_stream_error("outgoing bidirectional streams ended".into())
            })?
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send).map_err(crate::error::internal_stream_error)?,
            recv: RecvStream::new(recv).map_err(crate::error::internal_stream_error)?,
        }))
    }

    fn poll_open_send(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::SendStream, StreamErrorIncoming>> {
        let uni = self.opening_uni.get_or_insert_with(|| {
            Box::pin(stream::unfold(self.conn.clone(), |conn| async {
                Some((conn.open_uni().await, conn))
            }))
        });

        let send = ready!(uni.poll_next_unpin(cx))
            .ok_or_else(|| {
                crate::error::internal_stream_error("outgoing unidirectional streams ended".into())
            })?
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Self::SendStream::new(send).map_err(crate::error::internal_stream_error))
    }

    fn close(&mut self, code: Code, reason: &[u8]) {
        close_connection(&self.conn, code, reason);
    }
}

fn close_connection(conn: &iroh::endpoint::Connection, code: Code, reason: &[u8]) {
    match VarInt::from_u64(code.value()) {
        Ok(code) => conn.close(code, reason),
        Err(_) => conn.close(
            VarInt::from_u32(0x102),
            b"invalid HTTP/3 application error code",
        ),
    }
}

impl Clone for OpenStreams {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            opening_bi: None,
            opening_uni: None,
        }
    }
}
