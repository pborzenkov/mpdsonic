use axum::{
    body::Body,
    extract::{FromRequest, RequestParts},
    response::{IntoResponse, Response},
};
use futures::future::Map;
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

// Trait for async functions that can be used to handle requests and return serializable reply.
pub trait Handler<T>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    // Call the handler with the given request
    fn call(self, req: http::Request<Body>) -> Self::Future;

    // Convert the handler into tower_service::Service
    fn into_service(self) -> IntoService<Self, T> {
        IntoService::new(self)
    }
}

// Trait for async functions that can be used to handle requests and return raw reply.
pub trait RawHandler<T>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    // Call the handler with the given request
    fn call(self, req: http::Request<Body>) -> Self::Future;

    // Convert the handler into tower_service::Service
    fn into_service(self) -> RawIntoService<Self, T> {
        RawIntoService::new(self)
    }
}

// Implement Handler trait for all async functions that take 0 arguments and return api::Result<T>
// where T can be transformed into Reply
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

            let format = super::serialization_format(&req);
            match self().await {
                Ok(reply) => super::serialize_reply(reply.into_reply(), &format),
                Err(error) => super::serialize_reply(error, &format),
            }
        })
    }
}

macro_rules! impl_handler {
    // Implement Handler trait for all async functions that take 1 or more arguments and return api::Result<T>
    // where T can be transformed into Reply
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<F, Fut, IR, R, $($ty,)*> Handler<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = super::Result<IR>> + Send,
            IR: IntoReply<Reply = R>,
            R: super::Reply,
            $($ty: FromRequest<Body> + Send,)*
            $(<$ty as FromRequest<Body>>::Rejection: Into<super::Error>,)*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: http::Request<Body>) -> Self::Future {
                Box::pin(async move {
                    let mut req = RequestParts::new(req);

                    let format = super::serialization_format(&req);

                    $(
                        let $ty = match $ty::from_request(&mut req).await {
                            Ok(value) => value,
                            Err(rejection) => return super::serialize_reply::<super::Error>(rejection.into(), &format),
                        };
                    )*

                    match self($($ty,)*).await {
                        Ok(reply) => super::serialize_reply(reply.into_reply(), &format),
                        Err(error) => super::serialize_reply(error, &format),
                    }
                })
            }
        }

        // Implement Handler trait for all async functions that take 1 or more arguments and return api::Result<T>
        // where T can be transformed into Response
        //
        // TODO: Drop once negative impls are stabilized
        #[allow(non_snake_case)]
        impl<F, Fut, IR, $($ty,)*> RawHandler<($($ty,)*)> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = super::Result<IR>> + Send,
            IR: IntoResponse,
            $($ty: FromRequest<Body> + Send,)*
            $(<$ty as FromRequest<Body>>::Rejection: Into<super::Error>,)*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: http::Request<Body>) -> Self::Future {
                Box::pin(async move {
                    let mut req = RequestParts::new(req);

                    let format = super::serialization_format(&req);

                    $(
                        let $ty = match $ty::from_request(&mut req).await {
                            Ok(value) => value,
                            Err(rejection) => return super::serialize_reply::<super::Error>(rejection.into(), &format),
                        };
                    )*

                    match self($($ty,)*).await {
                        Ok(response) => response.into_response(),
                        Err(error) => super::serialize_reply(error, &format),
                    }
                })
            }
        }
    };
}

impl_handler!(T1);
impl_handler!(T1, T2);

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
        use futures::future::FutureExt;

        H::call(self.handler.clone(), req).map(Ok)
    }
}

// An adapter that makes RawHandler into tower_service::Service
#[derive(Clone)]
pub struct RawIntoService<H, T> {
    handler: H,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T> RawIntoService<H, T> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            _marker: PhantomData,
        }
    }
}

impl<H, T> Service<http::Request<Body>> for RawIntoService<H, T>
where
    H: RawHandler<T>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Map<H::Future, fn(Response) -> Result<Response, Infallible>>;

    fn poll_ready(&mut self, _ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        use futures::future::FutureExt;

        H::call(self.handler.clone(), req).map(Ok)
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
