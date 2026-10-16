use crate::{
    VarInt,
    error::{convert_read_error_to_stream_error, convert_write_error_to_stream_error},
};
use bytes::{Buf, Bytes};
use futures::ready;
use h3::quic::{self, ConnectionErrorIncoming, StreamErrorIncoming, StreamId, WriteBuf};
use std::{
    future::Future,
    pin::Pin,
    task::{self, Poll},
};
/// A bidirectional stream composed of a [`SendStream`] and a [`RecvStream`].
pub struct BidiStream<B>
where
    B: Buf,
{
    pub(crate) send: SendStream<B>,
    pub(crate) recv: RecvStream,
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

/// A receive-only half of an Iroh QUIC stream, implementing the h3 `RecvStream` trait.
pub struct RecvStream {
    stream: iroh::endpoint::RecvStream,
}

impl RecvStream {
    pub(crate) fn new(stream: iroh::endpoint::RecvStream) -> Self {
        Self { stream }
    }
}

impl quic::RecvStream for RecvStream {
    type Buf = Bytes;

    fn poll_data(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Bytes>, StreamErrorIncoming>> {
        // read_chunk is cancellation-safe; all receive state lives in the stream.
        let read = std::pin::pin!(self.stream.read_chunk(usize::MAX));
        Poll::Ready(ready!(read.poll(cx)).map_err(convert_read_error_to_stream_error))
    }

    fn stop_sending(&mut self, error_code: u64) {
        let _ = self
            .stream
            .stop(VarInt::from_u64(error_code).unwrap_or(VarInt::MAX));
    }

    fn recv_id(&self) -> StreamId {
        let num: u64 = self.stream.id().into();
        num.try_into().expect("invalid stream id")
    }
}
/// A send-only half of an Iroh QUIC stream, implementing the h3 `SendStream` trait.
pub struct SendStream<B: Buf> {
    stream: iroh::endpoint::SendStream,
    writing: Option<WriteBuf<B>>,
}

impl<B> SendStream<B>
where
    B: Buf,
{
    pub(crate) fn new(stream: iroh::endpoint::SendStream) -> SendStream<B> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use h3::quic::RecvStream as _;
    use iroh::{Endpoint, endpoint::presets};

    #[tokio::test]
    async fn a_pending_zero_copy_read_can_be_identified_cancelled_and_resumed() {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let server = Endpoint::builder(presets::Minimal)
                .alpns(vec![b"test".to_vec()])
                .bind()
                .await
                .unwrap();
            let client = Endpoint::builder(presets::Minimal).bind().await.unwrap();
            let (client_conn, server_conn) =
                tokio::join!(client.connect(server.addr(), b"test"), async {
                    server.accept().await.unwrap().await
                });
            let client_conn = client_conn.unwrap();
            let server_conn = server_conn.unwrap();
            let (mut send, _) = client_conn.open_bi().await.unwrap();
            send.write_all(b"one").await.unwrap();
            let (_, recv) = server_conn.accept_bi().await.unwrap();
            let mut recv = RecvStream::new(recv);
            assert_eq!(
                std::future::poll_fn(|cx| recv.poll_data(cx))
                    .await
                    .unwrap()
                    .unwrap(),
                b"one".as_slice()
            );
            let waker = futures::task::noop_waker();
            let mut cx = task::Context::from_waker(&waker);
            assert!(recv.poll_data(&mut cx).is_pending());
            let _id = recv.recv_id();
            send.write_all(b"two").await.unwrap();
            assert_eq!(
                std::future::poll_fn(|cx| recv.poll_data(cx))
                    .await
                    .unwrap()
                    .unwrap(),
                b"two".as_slice()
            );
            assert!(recv.poll_data(&mut cx).is_pending());
            recv.stop_sending(42);
            assert_eq!(send.stopped().await.unwrap().unwrap().into_inner(), 42);
            client.close().await;
            server.close().await;
        })
        .await
        .expect("QUIC receive test stalled");
    }
}
