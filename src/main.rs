use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use mpd_client::{commands::SetBinaryLimit, Client};
use std::{
    env,
    net::{self, SocketAddr},
};
use tokio::net::TcpStream;
use tracing::{debug, warn};

mod api;
mod app;
mod library;
mod mpd;

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
    let connection = TcpStream::connect(
        matches
            .get_one::<SocketAddr>("mpd-address")
            .expect("`mpd-address` is required"),
    )
    .await
    .unwrap();
    let (client, _) = Client::connect_with_password_opt(
        connection,
        matches
            .get_one::<String>("mpd-password")
            .map(String::as_str),
    )
    .await
    .unwrap();

    client.command(SetBinaryLimit(128 * 1024)).await.unwrap();

    let auth = api::Authentication::new(
        matches
            .get_one::<String>("username")
            .expect("`username` is required"),
        matches
            .get_one::<String>("password")
            .expect("`password` is required"),
    );
    let app = api::get_router(
        auth,
        client,
        library::Library::new(
            matches
                .get_one::<String>("mpd-library")
                .expect("`mpd-library` is required"),
        )
        .unwrap(),
    )
    .layer(middleware::from_fn(print_request));

    axum::Server::bind(
        &(
            *matches
                .get_one::<net::IpAddr>("address")
                .expect("`address` is required"),
            *matches.get_one::<u16>("port").expect("`port` is required"),
        )
            .into(),
    )
    .serve(app.into_make_service())
    .await
    .unwrap();
}
