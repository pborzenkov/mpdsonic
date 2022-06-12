use axum::{body::Body, extract::RequestParts, response::Response};
use futures_util::future::Map;
use serde::Serialize;
use std::{
    convert::Infallible,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;
use yaserde_derive::YaSerialize;

// Trait for async functions that can be used to handle requests.
pub trait Handler<T>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    // Call the handler with the given request
    fn call(self, req: http::Request<Body>) -> Self::Future;

    // Convert the handler into tower_service::Service
    fn into_service(self) -> IntoService<Self, T> {
        IntoService::new(self)
    }
}

// Implement Handler trait for all async functions that take 0 arguments and return api::Result<T>
impl<F, Fut, IR, R> Handler<()> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = super::Result<IR>> + Send,
    IR: IntoReply<Reply = R>,
    R: super::Reply,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: http::Request<Body>) -> Self::Future {
        Box::pin(async move {
            let req = RequestParts::new(req);

            let format = serialization_format(&req);
            match self().await {
                Ok(reply) => super::serialize_reply(reply.into_reply(), &format),
                Err(error) => super::serialize_reply(error, &format),
            }
        })
    }
}

fn serialization_format(req: &RequestParts<Body>) -> super::SerializationFormat {
    let query = req.uri().query().unwrap_or_default();

    // Official Subsonic server falls back to XML if some of the parameters are invalid or not provided
    serde_urlencoded::from_str::<super::SerializationFormat>(query).unwrap_or_default()
}

// An adapter that makes Handler into tower_service::Service
#[derive(Clone)]
pub struct IntoService<H, T> {
    handler: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T> IntoService<H, T> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T> Service<http::Request<Body>> for IntoService<H, T>
where
    H: Handler<T>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Map<H::Future, fn(Response) -> Result<Response, Infallible>>;

    fn poll_ready(&mut self, _ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        use futures_util::future::FutureExt;

        Handler::call(self.handler.clone(), req).map(Ok)
    }
}

#[derive(Serialize, YaSerialize)]
struct Empty;

impl super::Reply for Empty {
    fn field_name() -> Option<&'static str> {
        None
    }
}

// Trait for data that can be converted into api::Reply
trait IntoReply {
    type Reply;

    fn into_reply(self) -> Self::Reply;
}

// IntoReply is implemented trivially for all replies
impl<T> IntoReply for T
where
    T: super::Reply,
{
    type Reply = T;
    fn into_reply(self) -> Self::Reply {
        self
    }
}

// And also for an empty tuple by returning a struct that
// also implements YaSerialize, which isn't implemented on an empty tuple :(
impl IntoReply for () {
    type Reply = Empty;
    fn into_reply(self) -> Self::Reply {
        Empty
    }
}
#[cfg(test)]
mod tests {
    use super::Empty;
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};

    #[test]
    fn error() {
        assert_eq!(xml(&Empty), expect_ok_xml(None));

        assert_eq!(json(&Empty), expect_ok_json(None));
    }
}
