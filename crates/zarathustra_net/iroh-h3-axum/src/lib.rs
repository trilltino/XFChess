#![deny(missing_docs)]

use std::{
    error::Error,
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
use n0_future::task; // unifies wasm/tokio task spawning.
use tower_service::Service;

type H3ServerConnection = server::Connection<IrohH3Connection, Bytes>;

#[derive(Debug)]
pub struct IrohAxum {
    router: Router,
}

impl IrohAxum {
    #[inline]
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    fn handle_request(
        &self,
        remote_id: EndpointId,
        request_resolver: RequestResolver<IrohH3Connection, Bytes>,
    ) {
        let router = self.router.clone();

        task::spawn(async move {
            let mut router = router;

            let (request, stream) = request_resolver.resolve_request().await?;

            // Extract request parts (headers, method, etc.).
            let parts = request.into_parts().0;

            // Split the bidirectional stream into sender and receiver halves.
            let (mut send, recv) = stream.split();

            // Wrap the receive half in an Axum-compatible body.
            let request_body = RequestBody { inner: recv };
            let mut request = Request::from_parts(parts, request_body);
            request.extensions_mut().insert(RemoteId(remote_id));

            // Call into the Axum router.
            let response = router.call(request).await?;

            // Send response headers.
            let (parts, body) = response.into_parts();
            let response_head: Response<()> = Response::from_parts(parts, ());
            send.send_response(response_head).await?;

            // Stream response body frames.
            let mut response_stream = body.into_data_stream();
            while let Some(Ok(chunk)) = response_stream.next().await {
                send.send_data(chunk).await?;
            }

            // Gracefully finish the response.
            send.finish().await?;

            Ok::<(), Box<dyn Error + Send + Sync>>(())
        });
    }
}

impl ProtocolHandler for IrohAxum {
    async fn accept(&self, connection: iroh::endpoint::Connection) -> Result<(), AcceptError> {
        let remote_id = connection.remote_id();
        let connection = IrohH3Connection::new(connection);
        let mut connection = H3ServerConnection::new(connection)
            .await
            .map_err(AcceptError::from_err)?;

        while let Some(request_resolver) =
            connection.accept().await.map_err(AcceptError::from_err)?
        {
            self.handle_request(remote_id, request_resolver);
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
