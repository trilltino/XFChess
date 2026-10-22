#[cfg(test)]
use anyhow::Context as _;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use bytes::Buf;
use bytes::Bytes;
use dashmap::{DashMap, Entry};
use futures::FutureExt;
use futures::future::Shared;
use h3::client::Connection;
use h3::client::RequestStream;
use http::Response;
use http::Uri;
use http::Version;
use http_body_util::BodyExt;
use iroh::{Endpoint, EndpointId};
use iroh_h3::BidiStream;
use iroh_h3::{Connection as IrohH3Connection, OpenStreams};
use n0_future::task;
use tracing::instrument;
use tracing::trace;
use tracing::warn;

use crate::body::Body;
use crate::error::Error;
use crate::error::RequestValidationError;
use crate::response::IrohH3ResponseBody;
use crate::service::Service;

type Sender = h3::client::SendRequest<OpenStreams, Bytes>;
type SenderFuture = Pin<Box<dyn futures::Future<Output = Result<Sender, Arc<Error>>> + Send>>;
type CachedSender = Shared<SenderFuture>;
type SenderCache = DashMap<EndpointId, (u64, CachedSender)>;

#[derive(Clone, Debug)]
pub struct ConnectionManager {
    endpoint: Endpoint,
    alpn: Vec<u8>,
    sender_cache: Arc<SenderCache>,
    generation: Arc<AtomicU64>,
    lifetime: Arc<tokio::sync::watch::Sender<()>>,
}

impl ConnectionManager {
    pub fn new(endpoint: Endpoint, alpn: Vec<u8>) -> Self {
        Self {
            endpoint,
            alpn,
            sender_cache: Default::default(),
            generation: Default::default(),
            lifetime: Arc::new(tokio::sync::watch::channel(()).0),
        }
    }

    pub async fn get_sender(&self, peer_id: EndpointId) -> Result<Sender, Error> {
        let (generation, shared) = match self.sender_cache.entry(peer_id) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let generation = self.generation.fetch_add(1, Ordering::Relaxed);
                let shared = self.create_connection(peer_id, generation);
                entry.insert((generation, shared.clone()));
                (generation, shared)
            }
        };
        match shared.await {
            Ok(sender) => Ok(sender),
            Err(error) => {
                self.sender_cache
                    .remove_if(&peer_id, |_, (cached, _)| *cached == generation);
                Err(error.into())
            }
        }
    }
    #[instrument(skip(self, peer_id))]
    fn create_connection(&self, peer_id: EndpointId, generation: u64) -> Shared<SenderFuture> {
        let endpoint = self.endpoint.clone();
        let alpn = self.alpn.clone();
        let cache = Arc::downgrade(&self.sender_cache);
        let lifetime = self.lifetime.subscribe();
        let fut: SenderFuture = Box::pin(async move {
            let conn = endpoint
                .connect(peer_id, &alpn)
                .await
                .map_err(|err| Error::Transport(err.into()))
                .map_err(Arc::new)?;
            let conn = IrohH3Connection::new(conn);
            let (conn, sender) = h3::client::new(conn)
                .await
                .map_err(|err| Error::Transport(err.into()))
                .map_err(Arc::new)?;

            task::spawn(Self::run_connection(
                conn, peer_id, generation, cache, lifetime,
            ));

            Ok(sender)
        });

        fut.shared()
    }

    #[instrument(skip(conn, cache, lifetime))]
    async fn run_connection(
        mut conn: Connection<iroh_h3::Connection, Bytes>,
        peer_id: EndpointId,
        generation: u64,
        cache: Weak<SenderCache>,
        mut lifetime: tokio::sync::watch::Receiver<()>,
    ) {
        tokio::select! {
            error = conn.wait_idle() => trace!("Connection with {} closed. Cause: {error}", peer_id.fmt_short()),
            _ = lifetime.changed() => {},
        }
        if let Some(cache) = cache.upgrade() {
            cache.remove_if(&peer_id, |_, (cached, _)| *cached == generation);
        }
    }

    #[instrument(skip(stream, body))]
    async fn send_body(
        stream: &mut RequestStream<BidiStream<Bytes>, Bytes>,
        body: Body,
    ) -> Result<(), Error> {
        let mut body_stream = body.into_stream();
        while let Some(frame) = body_stream.frame().await.transpose()? {
            match frame.into_data() {
                Ok(mut data) => {
                    let buf = data.copy_to_bytes(data.remaining());
                    stream
                        .send_data(buf)
                        .await
                        .map_err(|err| Error::Transport(err.into()))?;
                }
                Err(frame) => match frame.into_trailers() {
                    Ok(trailers) => stream
                        .send_trailers(trailers)
                        .await
                        .map_err(|err| Error::Transport(err.into()))?,
                    Err(_) => warn!("Unexpected frame type"),
                },
            }
        }
        Ok(())
    }
}

impl Service for ConnectionManager {
    #[instrument(skip(self, request))]
    async fn handle(&self, mut request: http::Request<Body>) -> Result<Response<Body>, Error> {
        let peer_id = peer_id(request.uri())?;
        let mut sender = self.get_sender(peer_id).await?;

        *request.version_mut() = Version::HTTP_3;

        let (parts, body) = request.into_parts();
        let req = http::Request::from_parts(parts, ());

        let mut stream = sender
            .send_request(req)
            .await
            .map_err(|err| Error::Transport(err.into()))?;
        Self::send_body(&mut stream, body).await?;
        stream
            .finish()
            .await
            .map_err(|err| Error::Transport(err.into()))?;

        let response = stream
            .recv_response()
            .await
            .map_err(|err| Error::Transport(err.into()))?;
        let inner = response.into_parts().0;

        let response_body = IrohH3ResponseBody::new(stream, sender, self.lifetime.clone());
        let boxed_response_body = response_body.boxed();

        Ok(Response::from_parts(inner, boxed_response_body.into()))
    }
}

#[instrument]
pub(crate) fn peer_id(uri: &Uri) -> Result<EndpointId, Error> {
    let authority = uri
        .authority()
        .ok_or(RequestValidationError::MissingAuthority)?
        .as_str();
    authority
        .parse()
        .map_err(|err| RequestValidationError::BadPeerId(err).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn abandoning_a_cached_connection_setup_releases_the_cache() -> anyhow::Result<()> {
        let endpoint = Endpoint::bind(iroh::endpoint::presets::Minimal)
            .await
            .context("required operation value")?;
        let manager = ConnectionManager::new(endpoint.clone(), b"test".to_vec());
        let cache = Arc::downgrade(&manager.sender_cache);
        let peer = iroh::SecretKey::generate().public();
        manager
            .sender_cache
            .insert(peer, (0, manager.create_connection(peer, 0)));
        drop(manager);
        assert!(
            cache.upgrade().is_none(),
            "cached setup retained its own cache"
        );
        endpoint.close().await;
        Ok(())
    }
}
