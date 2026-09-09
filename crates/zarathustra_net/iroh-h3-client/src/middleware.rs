pub mod cookie_jar;
pub mod follow_redirects;
pub mod retry_failures;
pub mod timeout;

use futures::future::BoxFuture;
use http::{Request, Response};
use mockall::automock;
use std::future::Future;
use std::sync::Arc;

use crate::{body::Body, error::Error};

#[automock]
pub trait Service: Send + Sync {
    fn handle(
        &self,
        request: Request<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>> + Send;
}

pub trait Middleware: Send + Sync {
    fn handle(
        &self,
        request: Request<Body>,
        next: &impl Service,
    ) -> impl Future<Output = Result<Response<Body>, Error>> + Send;
}

impl<M, S> Service for (&M, &S)
where
    M: Middleware,
    S: Service,
{
    fn handle(
        &self,
        request: Request<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>> + Send {
        self.0.handle(request, self.1)
    }
}

impl<M1, M2> Middleware for (M1, M2)
where
    M1: Middleware,
    M2: Middleware,
{
    async fn handle(
        &self,
        request: Request<Body>,
        next: &impl Service,
    ) -> Result<Response<Body>, Error> {
        // Compose mw2 + next into a temporary service
        let composed = (&self.1, next);

        // First call mw1, with mw2 as its "next"
        self.0.handle(request, &composed).await
    }
}

type ServiceFuture = BoxFuture<'static, Result<Response<Body>, Error>>;

pub struct Pipeline {
    inner: Box<dyn Fn(Request<Body>) -> ServiceFuture + Send + Sync>,
}

impl Pipeline {
    pub fn new(service: impl Service + 'static) -> Self {
        let arc = Arc::new(service);
        Self {
            inner: Box::new(move |request| {
                let clone = arc.clone();
                Box::pin(async move { clone.handle(request).await })
            }),
        }
    }

    pub fn with_middleware(
        middleware: impl Middleware + 'static,
        service: impl Service + 'static,
    ) -> Self {
        let middleware_arc = Arc::new(middleware);
        let service_arc = Arc::new(service);
        Self {
            inner: Box::new(move |request| {
                let middleware_clone = middleware_arc.clone();
                let service_clone = service_arc.clone();
                Box::pin(async move {
                    let service = (middleware_clone.as_ref(), service_clone.as_ref());
                    service.handle(request).await
                })
            }),
        }
    }
}

impl Service for Pipeline {
    fn handle(
        &self,
        request: Request<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>> + Send {
        (self.inner)(request)
    }
}
