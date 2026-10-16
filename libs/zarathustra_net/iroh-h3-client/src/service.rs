use crate::{body::Body, error::Error};
use http::{Request, Response};
use std::future::Future;

#[cfg_attr(test, mockall::automock)]
pub trait Service: Send + Sync {
    fn handle(
        &self,
        request: Request<Body>,
    ) -> impl Future<Output = Result<Response<Body>, Error>> + Send;
}
