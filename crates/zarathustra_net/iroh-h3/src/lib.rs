#![deny(missing_docs)]

use std::{
    convert::TryInto,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{self, Poll},
};

use bytes::{Buf, Bytes};
use futures::{
    Stream, StreamExt, ready,
    stream::{self},
};
use h3::{
    error::Code,
    quic::{self, ConnectionErrorIncoming, StreamErrorIncoming, StreamId, WriteBuf},
};
pub use iroh::endpoint::{AcceptBi, AcceptUni, Endpoint, OpenBi, OpenUni, VarInt};
use iroh::endpoint::{ConnectionError, ReadError, WriteError};
use tokio_util::sync::ReusableBoxFuture;

type BoxStreamSync<'a, T> = Pin<Box<dyn Stream<Item = T> + Sync + Send + 'a>>;

pub struct Connection {
    conn: iroh::endpoint::Connection,
    incoming_bi: BoxStreamSync<'static, <AcceptBi<'static> as Future>::Output>,
    opening_bi: Option<BoxStreamSync<'static, <OpenBi<'static> as Future>::Output>>,
    incoming_uni: BoxStreamSync<'static, <AcceptUni<'static> as Future>::Output>,
    opening_uni: Option<BoxStreamSync<'static, <OpenUni<'static> as Future>::Output>>,
}

impl Connection {
    pub fn new(conn: iroh::endpoint::Connection) -> Self {
        Self {
            conn: conn.clone(),
            incoming_bi: Box::pin(stream::unfold(conn.clone(), |conn| async {
                Some((conn.accept_bi().await, conn))
            })),
            opening_bi: None,
            incoming_uni: Box::pin(stream::unfold(conn.clone(), |conn| async {
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
            .expect("self.incoming_bi BoxStream never returns None")
            .map_err(convert_connection_error)?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send),
            recv: Self::RecvStream::new(recv),
        }))
    }

    fn poll_accept_recv(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::RecvStream, ConnectionErrorIncoming>> {
        let recv = ready!(self.incoming_uni.poll_next_unpin(cx))
            .expect("self.incoming_uni BoxStream never returns None")
            .map_err(convert_connection_error)?;
        Poll::Ready(Ok(Self::RecvStream::new(recv)))
    }

    fn opener(&self) -> Self::OpenStreams {
        OpenStreams {
            conn: self.conn.clone(),
            opening_bi: None,
            opening_uni: None,
        }
    }
}

fn convert_connection_error(e: ConnectionError) -> h3::quic::ConnectionErrorIncoming {
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
            .expect("BoxStream does not return None")
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send),
            recv: RecvStream::new(recv),
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
            .expect("BoxStream does not return None")
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::SendStream::new(send)))
    }

    fn close(&mut self, code: Code, reason: &[u8]) {
        self.conn.close(
            VarInt::from_u64(code.value()).expect("error code VarInt"),
            reason,
        );
    }
}

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
            .expect("BoxStream does not return None")
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::BidiStream {
            send: Self::SendStream::new(send),
            recv: RecvStream::new(recv),
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
            .expect("BoxStream does not return None")
            .map_err(|e| StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: convert_connection_error(e),
            })?;
        Poll::Ready(Ok(Self::SendStream::new(send)))
    }

    fn close(&mut self, code: Code, reason: &[u8]) {
        self.conn.close(
            VarInt::from_u64(code.value()).expect("error code VarInt"),
            reason,
        );
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

pub struct BidiStream<B>
where
    B: Buf,
{
    send: SendStream<B>,
    recv: RecvStream,
}

impl<B> quic::BidiStream<B> for BidiStream<B>
where
    B: Buf,
{
    type SendStream = SendStream<B>;
    type RecvStream = RecvStream;

    fn split(self) -> (Self::SendStream, Self::RecvStream) {
        (self.send, self.recv)
    }
}

impl<B: Buf> quic::RecvStream for BidiStream<B> {
    type Buf = Bytes;

    fn poll_data(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Self::Buf>, StreamErrorIncoming>> {
        self.recv.poll_data(cx)
    }

    fn stop_sending(&mut self, error_code: u64) {
        self.recv.stop_sending(error_code)
    }

    fn recv_id(&self) -> StreamId {
        self.recv.recv_id()
    }
}

impl<B> quic::SendStream<B> for BidiStream<B>
where
    B: Buf,
{
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
        self.send.poll_ready(cx)
    }

    fn poll_finish(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
        self.send.poll_finish(cx)
    }

    fn reset(&mut self, reset_code: u64) {
        self.send.reset(reset_code)
    }

    fn send_data<D: Into<WriteBuf<B>>>(&mut self, data: D) -> Result<(), StreamErrorIncoming> {
        self.send.send_data(data)
    }

    fn send_id(&self) -> StreamId {
        self.send.send_id()
    }
}

impl<B> quic::SendStreamUnframed<B> for BidiStream<B>
where
    B: Buf,
{
    fn poll_send<D: Buf>(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut D,
    ) -> Poll<Result<usize, StreamErrorIncoming>> {
        self.send.poll_send(cx, buf)
    }
}

pub struct RecvStream {
    stream: Option<iroh::endpoint::RecvStream>,
    read_chunk_fut: ReadChunkFuture,
}

type ReadChunkFuture = ReusableBoxFuture<
    'static,
    (
        iroh::endpoint::RecvStream,
        Result<Option<Bytes>, iroh::endpoint::ReadError>,
    ),
>;

impl RecvStream {
    fn new(stream: iroh::endpoint::RecvStream) -> Self {
        Self {
            stream: Some(stream),
            read_chunk_fut: ReusableBoxFuture::new(async { unreachable!() }),
        }
    }
}

impl quic::RecvStream for RecvStream {
    type Buf = Bytes;

    fn poll_data(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Self::Buf>, StreamErrorIncoming>> {
        if let Some(mut stream) = self.stream.take() {
            self.read_chunk_fut.set(async move {
                let chunk = stream.read_chunk(usize::MAX).await;
                (stream, chunk)
            })
        };

        let (stream, chunk) = ready!(self.read_chunk_fut.poll(cx));
        self.stream = Some(stream);
        Poll::Ready(Ok(chunk.map_err(convert_read_error_to_stream_error)?))
    }

    fn stop_sending(&mut self, error_code: u64) {
        self.stream
            .as_mut()
            .unwrap()
            .stop(VarInt::from_u64(error_code).expect("invalid error_code"))
            .ok();
    }

    fn recv_id(&self) -> StreamId {
        let num: u64 = self.stream.as_ref().unwrap().id().into();
        num.try_into().expect("invalid stream id")
    }
}

fn convert_read_error_to_stream_error(error: ReadError) -> StreamErrorIncoming {
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

fn convert_write_error_to_stream_error(error: WriteError) -> StreamErrorIncoming {
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

pub struct SendStream<B: Buf> {
    stream: iroh::endpoint::SendStream,
    writing: Option<WriteBuf<B>>,
}

impl<B> SendStream<B>
where
    B: Buf,
{
    fn new(stream: iroh::endpoint::SendStream) -> SendStream<B> {
        Self {
            stream,
            writing: None,
        }
    }
}

impl<B> quic::SendStream<B> for SendStream<B>
where
    B: Buf,
{
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
        if let Some(ref mut data) = self.writing {
            while data.has_remaining() {
                let stream = Pin::new(&mut self.stream);
                let written = ready!(stream.poll_write(cx, data.chunk()))
                    .map_err(convert_write_error_to_stream_error)?;
                data.advance(written);
            }
        }
        self.writing = None;
        Poll::Ready(Ok(()))
    }

    fn poll_finish(
        &mut self,
        _cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), StreamErrorIncoming>> {
        Poll::Ready(
            self.stream
                .finish()
                .map_err(|e| StreamErrorIncoming::Unknown(Box::new(e))),
        )
    }

    fn reset(&mut self, reset_code: u64) {
        let _ = self
            .stream
            .reset(VarInt::from_u64(reset_code).unwrap_or(VarInt::MAX));
    }

    fn send_data<D: Into<WriteBuf<B>>>(&mut self, data: D) -> Result<(), StreamErrorIncoming> {
        if self.writing.is_some() {
            return Err(StreamErrorIncoming::ConnectionErrorIncoming {
                connection_error: ConnectionErrorIncoming::InternalError(
                    "internal error in the http stack".to_string(),
                ),
            });
        }
        self.writing = Some(data.into());
        Ok(())
    }

    fn send_id(&self) -> StreamId {
        let num: u64 = self.stream.id().into();
        num.try_into().expect("invalid stream id")
    }
}

impl<B> quic::SendStreamUnframed<B> for SendStream<B>
where
    B: Buf,
{
    fn poll_send<D: Buf>(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut D,
    ) -> Poll<Result<usize, StreamErrorIncoming>> {
        if self.writing.is_some() {
            panic!("poll_send called while send stream is not ready");
        }

        let s = Pin::new(&mut self.stream);

        let res = ready!(s.poll_write(cx, buf.chunk()));
        match res {
            Ok(written) => {
                buf.advance(written);
                Poll::Ready(Ok(written))
            }
            Err(err) => Poll::Ready(Err(convert_write_error_to_stream_error(err))),
        }
    }
}
