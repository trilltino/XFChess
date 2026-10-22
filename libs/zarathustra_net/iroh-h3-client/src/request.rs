use bytes::Bytes;
use http::request::Builder;
use http::{HeaderValue, header::CONTENT_TYPE};
#[cfg(feature = "json")]
use serde::Serialize;
use tracing::instrument;

use crate::IrohH3Client;
use crate::body::Body;
use crate::service::Service;
use crate::{error::Error, response::Response};

pub type ClientRequest = Request<IrohH3Client>;

#[derive(Debug)]
#[must_use]
pub struct RequestBuilder<C: Service> {
    pub(crate) inner: Builder,
    pub(crate) client: C,
}

impl<C: Service> RequestBuilder<C> {
    #[inline]
    pub fn extension<T>(mut self, extension: T) -> Self
    where
        T: Clone + std::any::Any + Send + Sync + 'static,
    {
        self.inner = self.inner.extension(extension);
        self
    }

    #[inline]
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<http::HeaderName>,
        <K as TryInto<http::HeaderName>>::Error: Into<http::Error>,
        V: TryInto<http::HeaderValue>,
        <V as TryInto<http::HeaderValue>>::Error: Into<http::Error>,
    {
        self.inner = self.inner.header(key, value);
        self
    }

    #[inline]
    pub fn body(self, body: Body) -> Result<Request<C>, Error> {
        let request = self.inner.body(body)?;
        Ok(Request {
            inner: request,
            client: self.client,
        })
    }

    #[inline]
    pub fn build(self) -> Result<Request<C>, Error> {
        self.body(Body::empty())
    }

    #[inline]
    fn ensure_content_type(mut self, value: HeaderValue) -> Self {
        if self
            .inner
            .headers_ref()
            .is_none_or(|headers| headers.get(CONTENT_TYPE).is_none())
        {
            self.inner = self.inner.header(CONTENT_TYPE, value);
        }
        self
    }

    #[inline]
    pub fn text(self, text: impl AsRef<str>) -> Result<Request<C>, Error> {
        const MIME_TEXT: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");

        let body_bytes = Bytes::copy_from_slice(text.as_ref().as_bytes());
        self.ensure_content_type(MIME_TEXT)
            .body(Body::bytes(body_bytes))
    }

    #[inline]
    pub fn bytes(self, bytes: impl Into<Bytes>) -> Result<Request<C>, Error> {
        const MIME_BIN: HeaderValue = HeaderValue::from_static("application/octet-stream");

        self.ensure_content_type(MIME_BIN)
            .body(Body::bytes(bytes.into()))
    }

    #[cfg(feature = "json")]
    #[inline]
    pub fn json<T: Serialize>(self, data: &T) -> Result<Request<C>, Error> {
        const MIME_JSON: HeaderValue = HeaderValue::from_static("application/json");

        let body = serde_json::to_vec(data).map_err(|err| Error::RequestValidation(err.into()))?;
        self.ensure_content_type(MIME_JSON)
            .body(Body::bytes(Bytes::from(body)))
    }

    #[inline]
    #[instrument(skip(self))]
    pub async fn send(self) -> Result<Response, Error> {
        self.build()?.send().await
    }
}

#[must_use]
#[derive(Debug)]
pub struct Request<C: Service> {
    inner: http::Request<Body>,
    client: C,
}

impl<C: Service> Request<C> {
    #[inline]
    #[instrument(skip(self))]
    pub async fn send(self) -> Result<Response, Error> {
        let response = self.client.handle(self.inner).await?;
        Ok(response.into())
    }
}

impl<C: Service> From<Request<C>> for http::Request<Body> {
    fn from(value: Request<C>) -> Self {
        value.inner
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        body::Body,
        error::Error,
        request::{Request, RequestBuilder},
        service::{MockService, Service},
    };
    use anyhow::Context as _;
    use http::{HeaderValue, Request as HttpRequest, Response as HttpResponse};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    type SharedMockService = Arc<Mutex<MockService>>;

    impl Service for SharedMockService {
        async fn handle(&self, request: HttpRequest<Body>) -> Result<HttpResponse<Body>, Error> {
            self.lock().await.handle(request).await
        }
    }

    fn builder_with_mock() -> (RequestBuilder<SharedMockService>, SharedMockService) {
        let mock_service = Arc::new(Mutex::new(MockService::new()));
        let builder = RequestBuilder {
            inner: http::Request::builder(),
            client: mock_service.clone(),
        };
        (builder, mock_service)
    }

    async fn verify_body(
        mut request: Request<SharedMockService>,
        predicate: impl Fn(&[u8]) -> bool,
    ) -> anyhow::Result<()> {
        let body_bytes = request
            .inner
            .body_mut()
            .take()
            .into_bytes()
            .await
            .context("required operation value")?;
        assert!(predicate(&body_bytes));
        Ok(())
    }

    #[tokio::test]
    async fn set_header() -> anyhow::Result<()> {
        let (builder, _mock) = builder_with_mock();
        let builder = builder.header("X-Test", "value");
        let req = builder.build().context("required operation value")?;
        assert_eq!(
            req.inner.headers().get("X-Test"),
            Some(&HeaderValue::from_static("value"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn add_extension() -> anyhow::Result<()> {
        #[derive(Clone)]
        struct MyExtension;
        let (builder, _mock) = builder_with_mock();
        let builder = builder.extension(MyExtension);
        let req = builder.build().context("required operation value")?;
        assert!(req.inner.extensions().get::<MyExtension>().is_some());
        Ok(())
    }

    #[tokio::test]
    async fn text_body_sets_content_type() -> anyhow::Result<()> {
        let (builder, _mock) = builder_with_mock();
        let req = builder
            .text("hello world")
            .context("required operation value")?;
        assert_eq!(
            req.inner.headers().get("content-type"),
            Some(&HeaderValue::from_static("text/plain; charset=utf-8"))
        );
        verify_body(req, |b| b == b"hello world").await?;
        Ok(())
    }

    #[tokio::test]
    async fn bytes_body_sets_content_type() -> anyhow::Result<()> {
        let (builder, _mock) = builder_with_mock();
        let data = b"abc".to_vec();
        let req = builder
            .bytes(data.clone())
            .context("required operation value")?;
        assert_eq!(
            req.inner.headers().get("content-type"),
            Some(&HeaderValue::from_static("application/octet-stream"))
        );
        verify_body(req, |b| b == b"abc").await?;
        Ok(())
    }

    #[tokio::test]
    async fn build_empty_body() -> anyhow::Result<()> {
        let (builder, _mock) = builder_with_mock();
        let req = builder.build().context("required operation value")?;
        verify_body(req, |b| b.is_empty()).await?;
        Ok(())
    }

    #[tokio::test]
    async fn send_request_calls_service() -> anyhow::Result<()> {
        let (builder, mock_service) = builder_with_mock();

        {
            let mut mock = mock_service.lock().await;
            mock.expect_handle().times(1).returning(|mut req| {
                Box::pin(async move {
                    Ok(HttpResponse::builder()
                        .status(200)
                        .body(req.body_mut().take())?)
                })
            });
        }

        let req = builder.text("hello").context("required operation value")?;
        let resp = req.send().await.context("required operation value")?;
        assert_eq!(resp.status, 200);
        assert_eq!(
            resp.bytes()
                .await
                .context("required operation value")?
                .to_vec(),
            b"hello"
        );
        Ok(())
    }

    #[tokio::test]
    async fn ensure_content_type_no_overwriting() -> anyhow::Result<()> {
        let (builder, _mock) = builder_with_mock();
        let builder = builder.header("content-type", "custom/type");
        let req = builder.text("hello").context("required operation value")?;
        assert_eq!(
            req.inner.headers().get("content-type"),
            Some(&HeaderValue::from_static("custom/type"))
        );
        verify_body(req, |b| b == b"hello").await?;
        Ok(())
    }
}
