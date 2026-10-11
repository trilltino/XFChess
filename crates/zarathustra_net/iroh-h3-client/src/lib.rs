mod body;
mod connection_manager;
pub mod error;
pub mod request;
pub mod response;
pub mod service;

use std::sync::Arc;

use http::request::Builder;
use http::{Method, Uri};
use iroh::Endpoint;

use crate::body::Body;
use crate::connection_manager::ConnectionManager;
use crate::error::Error;
use crate::request::RequestBuilder;
use crate::service::Service;

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct IrohH3Client {
    inner: Arc<ConnectionManager>,
}

macro_rules! http_method {
    ($name:ident, $variant:expr) => {
        #[inline]
        pub fn $name<U>(&self, uri: U) -> RequestBuilder<Self>
        where
            U: TryInto<Uri>,
            http::Error: From<<U as TryInto<Uri>>::Error>,
        {
            self.request($variant, uri)
        }
    };
}

impl IrohH3Client {
    pub fn new(endpoint: Endpoint, alpn: Vec<u8>) -> Self {
        let connection_manager = ConnectionManager::new(endpoint, alpn);
        Self {
            inner: Arc::new(connection_manager),
        }
    }
    pub fn request<U>(&self, method: http::Method, uri: U) -> RequestBuilder<Self>
    where
        U: TryInto<Uri>,
        http::Error: From<<U as TryInto<Uri>>::Error>,
    {
        RequestBuilder {
            inner: Builder::new().method(method).uri(uri),
            client: self.clone(),
        }
    }

    http_method!(head, Method::HEAD);
    http_method!(get, Method::GET);
    http_method!(post, Method::POST);
    http_method!(put, Method::PUT);
    http_method!(patch, Method::PATCH);
    http_method!(delete, Method::DELETE);
}

impl Service for IrohH3Client {
    #[tracing::instrument(skip(self))]
    async fn handle(&self, request: http::Request<Body>) -> Result<http::Response<Body>, Error> {
        self.inner.handle(request).await
    }
}
