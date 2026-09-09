use std::{
    collections::VecDeque,
    pin::{Pin, pin},
    task::{Context, Poll, ready},
};

use bytes::{Buf, Bytes};
use futures::Stream;
use http_body::Body;
use http_body_util::combinators::BoxBody;
use serde::de::DeserializeOwned;
use serde_json::from_slice;
use tracing::instrument;

use crate::{error::Error, response::Response};

pub struct NdjsonStream<T> {
    body: BoxBody<Bytes, Error>,

    buffer: VecDeque<u8>,

    line_buf: Vec<u8>,

    queue: VecDeque<Result<T, Error>>,

    _marker: std::marker::PhantomData<T>,
}

impl<T> NdjsonStream<T>
where
    T: DeserializeOwned,
{
    pub fn new(response: Response) -> Self {
        Self {
            body: response.body.into_stream(),
            buffer: VecDeque::with_capacity(256),
            line_buf: Vec::with_capacity(256),
            queue: VecDeque::with_capacity(16),
            _marker: std::marker::PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_stream<S>(stream: S) -> Self
    where
        S: Stream<Item = Result<Bytes, Error>> + Send + Sync + 'static,
    {
        use futures::StreamExt;
        use http_body::Frame;
        use http_body_util::StreamBody;

        let stream = stream.map(|buf| Ok(Frame::data(buf?)));

        Self {
            body: BoxBody::new(StreamBody::new(stream)),
            buffer: VecDeque::with_capacity(256),
            line_buf: Vec::with_capacity(256),
            queue: VecDeque::with_capacity(16),
            _marker: std::marker::PhantomData,
        }
    }

    fn append_frame(&mut self, mut frame: impl Buf) {
        let old_len = self.buffer.len();

        // Copy incoming bytes into the main buffer
        while frame.has_remaining() {
            let chunk = frame.chunk();
            self.buffer.extend(chunk);
            frame.advance(chunk.len());
        }

        let mut processed = 0;

        // Scan for newline-delimited NDJSON lines
        for pos in old_len..self.buffer.len() {
            if self.buffer[pos] == b'\n' {
                let start = processed;
                let end = pos;

                // Copy the bytes for the NDJSON line into line_buf
                let mut buf = std::mem::take(&mut self.line_buf);
                buf.clear();
                buf.reserve(end - start);

                let (s1, s2) = self.buffer.as_slices();
                if end <= s1.len() {
                    buf.extend_from_slice(&s1[start..end]);
                } else if start >= s1.len() {
                    let s2_start = start - s1.len();
                    let s2_end = end - s1.len();
                    buf.extend_from_slice(&s2[s2_start..s2_end]);
                } else {
                    buf.extend_from_slice(&s1[start..]);
                    buf.extend_from_slice(&s2[..(end - s1.len())]);
                }

                // Attempt JSON deserialization
                let value = from_slice::<T>(&buf)
                    .map_err(From::from)
                    .map_err(Error::ResponseValidation);

                self.queue.push_back(value);

                // Reuse the buffer for later lines
                buf.clear();
                self.line_buf = buf;

                processed = pos + 1;
            }
        }

        // Discard processed bytes
        self.buffer.drain(0..processed);
    }
}

impl<T> Unpin for NdjsonStream<T> {}

impl<T> Stream for NdjsonStream<T>
where
    T: DeserializeOwned,
{
    type Item = Result<T, Error>;

    #[instrument(skip(self, cx))]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(item) = self.queue.pop_front() {
            return Poll::Ready(Some(item));
        }

        let body = pin!(&mut self.body);
        let poll = Body::poll_frame(body, cx);
        let frame_result = ready!(poll);

        let frame = match frame_result {
            Some(Ok(frame)) => frame,
            Some(Err(err)) => return Poll::Ready(Some(Err(err))),
            None => return Poll::Ready(None), // EOF
        };

        // Only handle DATA frames
        let Ok(data) = frame.into_data() else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };

        // Scan for newline-delimited JSON
        self.append_frame(data);

        if let Some(item) = self.queue.pop_front() {
            return Poll::Ready(Some(item));
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::{StreamExt, stream};
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Row {
        id: u64,
        #[serde(default)]
        name: String,
    }

    fn ok(bytes: &'static [u8]) -> Result<Bytes, Error> {
        Ok(Bytes::from_static(bytes))
    }

    #[tokio::test]
    async fn simple_row() {
        let frames = stream::iter(vec![ok(b"{\"id\":1}\n")]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);
        let row = nd.next().await.unwrap().unwrap();

        assert_eq!(
            row,
            Row {
                id: 1,
                name: "".into()
            }
        );
        assert!(nd.next().await.is_none());
    }

    #[tokio::test]
    async fn split_frames_recombine() {
        let frames = stream::iter(vec![ok(b"{\"id\""), ok(b":2}\n")]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);
        let row = nd.next().await.unwrap().unwrap();

        assert_eq!(row.id, 2);
    }

    #[tokio::test]
    async fn multiple_rows_in_one_frame() {
        let frames = stream::iter(vec![ok(b"{\"id\":1}\n{\"id\":2}\n")]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);

        let r1 = nd.next().await.unwrap().unwrap();
        let r2 = nd.next().await.unwrap().unwrap();

        assert_eq!(r1.id, 1);
        assert_eq!(r2.id, 2);
    }

    #[tokio::test]
    async fn unknown_fields_ignored() {
        let frames = stream::iter(vec![ok(b"{\"id\":3,\"foo\":123}\n")]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);
        let row = nd.next().await.unwrap().unwrap();

        assert_eq!(row.id, 3);
    }

    #[tokio::test]
    async fn malformed_json_propagates_error() {
        let frames = stream::iter(vec![ok(b"{bad json}\n")]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);

        let err = nd.next().await.unwrap().unwrap_err();

        match err {
            Error::ResponseValidation(_) => {}
            _ => panic!("expected ResponseValidation"),
        }
    }

    #[tokio::test]
    async fn incomplete_line_on_eof_discarded() {
        let frames = stream::iter(vec![
            ok(b"{\"id\":10"), // missing newline
        ]);

        let mut nd = NdjsonStream::<Row>::from_stream(frames);

        assert!(nd.next().await.is_none());
    }
}
