//! Axum integration for the iroh-h3 HTTP/3 transport layer.

#![deny(missing_docs)]

use std::{
    error::Error,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{Router, body::HttpBody, extract::FromRequestParts};
use bytes::{Buf, Bytes};
use futures_lite::StreamExt;
use h3::server::{self, RequestResolver, RequestStream};
use http::{Request, Response, StatusCode};
use http_body::Frame;
use iroh::{
    EndpointId,
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_h3::{Connection as IrohH3Connection, RecvStream};
use n0_future::task::JoinSet;
use tower_service::Service;

type H3ServerConnection = server::Connection<IrohH3Connection, Bytes>;

/// An Axum HTTP/3 server backed by an Iroh QUIC endpoint.
#[derive(Debug)]
pub struct IrohAxum {
    router: Router,
}

impl IrohAxum {
    /// Creates a new [`IrohAxum`] server wrapping the given Axum [`Router`].
    #[inline]
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    fn handle_request(
        &self,
        remote_id: EndpointId,
        request_resolver: RequestResolver<IrohH3Connection, Bytes>,
    ) -> impl Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static {
        let router = self.router.clone();

        async move {
            let mut router = router;

            let (request, stream) = request_resolver.resolve_request().await?;

            let parts = request.into_parts().0;

            let (mut send, recv) = stream.split();

            let request_body = RequestBody { inner: recv };
            let mut request = Request::from_parts(parts, request_body);
            request.extensions_mut().insert(RemoteId(remote_id));

            let response = router.call(request).await?;

            let (parts, body) = response.into_parts();
            let response_head: Response<()> = Response::from_parts(parts, ());
            send.send_response(response_head).await?;

            let mut response_stream = body.into_data_stream();
            while let Some(chunk) = response_stream.next().await {
                match chunk {
                    Ok(chunk) => send.send_data(chunk).await?,
                    Err(error) => {
                        send.stop_stream(h3::error::Code::H3_INTERNAL_ERROR);
                        return Err(error.into());
                    }
                }
            }

            send.finish().await?;

            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
    }
}

impl ProtocolHandler for IrohAxum {
    async fn accept(&self, connection: iroh::endpoint::Connection) -> Result<(), AcceptError> {
        let remote_id = connection.remote_id();
        let connection = IrohH3Connection::new(connection);
        let mut connection = H3ServerConnection::new(connection)
            .await
            .map_err(AcceptError::from_err)?;

        let mut requests = JoinSet::new();
        loop {
            tokio::select! {
                resolver = connection.accept() => {
                    let Some(resolver) = resolver.map_err(AcceptError::from_err)? else { break };
                    requests.spawn(self.handle_request(remote_id, resolver));
                }
                _ = requests.join_next(), if !requests.is_empty() => {},
            }
        }

        Ok(())
    }
}

#[repr(transparent)]
struct RequestBody {
    inner: RequestStream<RecvStream, Bytes>,
}

impl HttpBody for RequestBody {
    type Data = Bytes;
    type Error = h3::error::StreamError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let inner = self.get_mut();
        match inner.inner.poll_recv_data(cx) {
            Poll::Ready(Ok(Some(mut chunk))) => {
                let bytes = chunk.copy_to_bytes(chunk.remaining());
                Poll::Ready(Some(Ok(Frame::data(bytes))))
            }
            Poll::Ready(Ok(None)) => Poll::Ready(None),
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Extractor that provides the remote [`EndpointId`] of the connecting Iroh peer.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct RemoteId(pub EndpointId);

impl<S> FromRequestParts<S> for RemoteId
where
    S: Sync,
{
    type Rejection = (StatusCode, &'static str);

    #[inline]
    async fn from_request_parts(
        parts: &mut http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts.extensions.get().copied().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Trying to extract RemoteId outside iroh-h3-axum context",
        ))
    }
}
