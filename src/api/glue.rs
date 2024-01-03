use axum::{
    async_trait,
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, Request},
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
pub(crate) trait Handler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    // Call the handler with the given request
    fn call(self, req: Request<Body>, state: S) -> Self::Future;

    // Convert the handler into tower_service::Service
    fn into_service(self) -> IntoService<Self, T, ()> {
        IntoService::new(self, ())
    }
}

// Trait for async functions that can be used to handle requests and return raw reply.
pub(crate) trait RawHandler<T, S>: Clone + Send + Sized + 'static {
    type Future: Future<Output = Response> + Send + 'static;

    // Call the handler with the given request
    fn call(self, req: Request<Body>, state: S) -> Self::Future;

    // Convert the handler into tower_service::Service
    fn into_service(self) -> RawIntoService<Self, T, ()> {
        RawIntoService::new(self, ())
    }
}

// Implement Handler trait for all async functions that take 0 arguments and return api::Result<T>
// where T can be transformed into Reply
impl<F, Fut, S, IR, R> Handler<(), S> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = super::Result<IR>> + Send,
    IR: IntoReply<Reply = R>,
    R: super::Reply,
{
    type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

    fn call(self, req: Request<Body>, _state: S) -> Self::Future {
        Box::pin(async move {
            let (parts, _) = req.into_parts();

            let format = super::serialization_format(&parts);
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
        impl<F, Fut, S, IR, R, $($ty,)*> Handler<($($ty,)*), S> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = super::Result<IR>> + Send,
            IR: IntoReply<Reply = R>,
            R: super::Reply,
            S: Send + Sync + 'static,
            $($ty: FromRequestParts<S> + Send,)*
            $(<$ty as FromRequestParts<S>>::Rejection: Into<super::Error>,)*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<Body>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, _) = req.into_parts();

                    let format = super::serialization_format(&parts);

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
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
        impl<F, Fut, S, IR, $($ty,)*> RawHandler<($($ty,)*), S> for F
        where
            F: FnOnce($($ty,)*) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = super::Result<IR>> + Send,
            IR: IntoResponse,
            S: Send + Sync + 'static,
            $($ty: FromRequestParts<S> + Send,)*
            $(<$ty as FromRequestParts<S>>::Rejection: Into<super::Error>,)*
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request<Body>, state: S) -> Self::Future {
                Box::pin(async move {
                    let (mut parts, _) = req.into_parts();

                    let format = super::serialization_format(&parts);

                    $(
                        let $ty = match $ty::from_request_parts(&mut parts, &state).await {
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
impl_handler!(T1, T2, T3);

// An adapter that makes Handler into tower_service::Service
#[derive(Clone)]
pub(crate) struct IntoService<H, T, S> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T, S> IntoService<H, T, S> {
    fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, T, S> Service<Request<Body>> for IntoService<H, T, S>
where
    S: Clone + Send + Sync,
    H: Handler<T, S>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Map<H::Future, fn(Response) -> Result<Response, Infallible>>;

    fn poll_ready(&mut self, _ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        use futures::future::FutureExt;

        H::call(self.handler.clone(), req, self.state.clone()).map(Ok)
    }
}

// An adapter that makes RawHandler into tower_service::Service
#[derive(Clone)]
pub(crate) struct RawIntoService<H, T, S> {
    handler: H,
    state: S,
    _marker: PhantomData<fn() -> T>,
}

impl<H, T, S> RawIntoService<H, T, S> {
    fn new(handler: H, state: S) -> Self {
        Self {
            handler,
            state,
            _marker: PhantomData,
        }
    }
}

impl<H, T, S> Service<Request<Body>> for RawIntoService<H, T, S>
where
    S: Clone + Send + Sync,
    H: RawHandler<T, S>,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Map<H::Future, fn(Response) -> Result<Response, Infallible>>;

    fn poll_ready(&mut self, _ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        use futures::future::FutureExt;

        H::call(self.handler.clone(), req, self.state.clone()).map(Ok)
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

#[derive(Clone, Debug)]
pub(crate) struct RawQuery(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for RawQuery
where
    S: Send + Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts(req: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query = req.uri.query().map(|query| query.to_owned());
        Ok(Self(query))
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
