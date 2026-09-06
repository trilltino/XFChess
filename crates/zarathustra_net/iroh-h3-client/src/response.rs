//! HTTP/3 response handling.

#[cfg(feature = "json")]
pub mod ndjson;
pub mod sse;

use std::ops::Deref;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use bytes::{Buf, Bytes};
use futures::{Stream, StreamExt};
use http_body::Frame;
use http_body_util::BodyExt;
use iroh_h3::OpenStreams;
#[cfg(feature = "json")]
use serde::de::DeserializeOwned;
use tracing::{debug, instrument, trace};

use crate::body::Body;
use crate::error::Error;
use crate::response::sse::{SseEvent, SseStream};

/// HTTP/3 response with header access and buffered or streaming body readers.
#[must_use]
pub struct Response {
    pub(crate) inner: http::response::Parts,
    pub(crate) body: Body,
}

impl From<http::Response<Body>> for Response {
    fn from(value: http::Response<Body>) -> Self {
        let (inner, body) = value.into_parts();
        Self { inner, body }
    }
}

impl Deref for Response {
    type Target = http::response::Parts;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Response {
    /// Reads the full response body into a contiguous [`Bytes`] buffer.
    ///
    /// # Example
    /// ```rust
    /// # use iroh_h3_client::request::ClientRequest;
    ///
    /// # async fn example(request: ClientRequest) -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// let mut response = request.send().await?;
    /// let body = response.bytes().await?;
    /// println!("Response body: {:?}", body);
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn bytes(self) -> Result<Bytes, Error> {
        let mut buf = Vec::new();
        let mut stream = self.bytes_stream();

        while let Some(data) = stream.next().await.transpose()? {
            debug!("received {} bytes", data.len());
            buf.extend_from_slice(&data);
        }

        Ok(Bytes::from(buf))
    }

    /// Reads the full response body as UTF-8 text.
    ///
    /// # Example
    /// ```rust
    /// # use iroh_h3_client::request::ClientRequest;
    ///
    /// # async fn example(request: ClientRequest) -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// let mut response = request.send().await?;
    /// let text = response.text().await?;
    /// println!("Response: {}", text);
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn text(self) -> Result<String, Error> {
        let bytes = self.bytes().await?;
        let string = String::from_utf8(bytes.to_vec()).map_err(|err| {
            debug!("UTF-8 conversion failed");
            Error::ResponseValidation(err.utf8_error().into())
        })?;
        Ok(string)
    }

    /// Deserializes the response body as JSON.
    ///
    /// # Example
    /// ```rust
    /// #[derive(serde::Deserialize)]
    /// struct ApiResponse { message: String }
    ///
    /// # use iroh_h3_client::request::ClientRequest;
    ///
    /// # async fn example(request: ClientRequest) -> Result<(), Box<dyn std::error::Error>> {
    ///
    /// let mut response = request.send().await?;
    /// let data: ApiResponse = response.json().await?;
    /// println!("Message: {}", data.message);
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "json")]
    #[instrument(skip(self))]
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, Error> {
        let bytes = self.bytes().await?;
        let value =
            serde_json::from_slice(&bytes).map_err(|err| Error::ResponseValidation(err.into()))?;
        debug!("parsed JSON successfully");
        Ok(value)
    }

    /// Streams response body chunks without buffering the full body.
    ///
    /// # Example
    /// ```rust
    /// use futures::StreamExt;
    /// # use iroh_h3_client::request::ClientRequest;
    ///
    /// # async fn example(request: ClientRequest) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut response = request.send().await?;
    /// let mut stream = response.bytes_stream();
    ///
    /// while let Some(chunk) = stream.next().await.transpose()? {
    ///     println!("Received chunk: {:?}", chunk);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub fn bytes_stream(self) -> impl Stream<Item = Result<Bytes, Error>> {
        self.body.into_stream().into_data_stream()
    }

    /// Returns a stream of Server-Sent Events
    #[instrument(skip(self))]
    pub fn sse_stream(self) -> impl Stream<Item = Result<SseEvent, Error>> {
        SseStream::new(self)
    }

    /// Returns a stream of NDJSON
    #[cfg(feature = "json")]
    #[instrument(skip(self))]
    pub fn ndjson_stream<T: DeserializeOwned>(self) -> impl Stream<Item = Result<T, Error>> {
        use crate::response::ndjson::NdjsonStream;

        NdjsonStream::new(self)
    }
}

/// HTTP/3 body implementing `http_body::Body`.
///
/// Wraps the `RequestStream` returned by iroh-h3.
pub(crate) struct IrohH3ResponseBody {
    pub(crate) stream: h3::client::RequestStream<iroh_h3::BidiStream<Bytes>, Bytes>,
    pub(crate) _sender: h3::client::SendRequest<OpenStreams, Bytes>,
}

impl IrohH3ResponseBody {
    pub(crate) fn new(
        stream: h3::client::RequestStream<iroh_h3::BidiStream<Bytes>, Bytes>,
        sender: h3::client::SendRequest<OpenStreams, Bytes>,
    ) -> Self {
        Self {
            stream,
            _sender: sender,
        }
    }
}

type BodyStreamItem = Result<http_body::Frame<Bytes>, Error>;

impl http_body::Body for IrohH3ResponseBody {
    type Data = Bytes;
    type Error = Error;

    #[instrument(skip(self, cx))]
    fn poll_frame(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<BodyStreamItem>> {
        match ready!(self.stream.poll_recv_data(cx)).transpose() {
            Some(Ok(mut frame)) => {
                trace!("received a frame of {} bytes", frame.remaining());
                let bytes = frame.copy_to_bytes(frame.remaining());
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Some(Err(err)) => {
                if err.is_h3_no_error() {
                    debug!("received H3_NO_ERROR");
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Err(Error::Transport(err.into()))))
                }
            }
            None => Poll::Ready(None),
        }
    }
}
