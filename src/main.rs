use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use mpd_client::Client;
use std::{env, net};
use tokio::net::TcpStream;
use tracing::{debug, warn};

mod api;
mod app;

async fn print_request(req: Request<Body>, next: Next<Body>) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;

    match response.status().is_success() {
        true => debug!(method = ?method, URI = ?uri, status = ?response.status()),
        _ => warn!(method = ?method, URI = ?uri, status = ?response.status()),
    }

    response
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches = app::build_app().get_matches_from(env::args_os());
    let connection = TcpStream::connect(matches.value_of("mpd-address").unwrap())
        .await
        .unwrap();
    let (client, _) =
        Client::connect_with_password_opt(connection, matches.value_of("mpd-password"))
            .await
            .unwrap();

    let auth = api::Authentication::new(
        matches.value_of("username").unwrap(),
        matches.value_of("password").unwrap(),
    );
    let app = api::get_router(auth, client).layer(middleware::from_fn(print_request));

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
