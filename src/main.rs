mod api;
mod app;

use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use std::{env, net};
use tracing::debug;

async fn print_request(req: Request<Body>, next: Next<Body>) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;

    debug!(method = ?method, URI = ?uri, status = ?response.status());

    response
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches = app::build_app().get_matches_from(env::args_os());
    let auth = api::Authentication::new(
        matches.value_of("username").unwrap(),
        matches.value_of("password").unwrap(),
    );
    let app = api::get_router(auth).layer(middleware::from_fn(print_request));

    axum::Server::bind(
        &(
            matches.value_of_t_or_exit::<net::IpAddr>("address"),
            matches.value_of_t_or_exit::<u16>("port"),
        )
            .into(),
    )
    .serve(app.into_make_service())
    .await
    .unwrap();
}
