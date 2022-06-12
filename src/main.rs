mod api;

use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::IntoResponse,
};
use clap::{crate_version, Arg, Command};
use std::{env, net};
use tracing::debug;

fn build_app() -> Command<'static> {
    Command::new("mpdsonic")
        .version(crate_version!())
        .arg(
            Arg::new("address")
                .long("address")
                .short('a')
                .help("The network address to listen to")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .help("The network port to listen to")
                .default_value("3000"),
        )
}

async fn print_request(req: Request<Body>, next: Next<Body>) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;

    debug!(method = ?method, URI = ?uri, status = ?response.status());

    response
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let matches = build_app().get_matches_from(env::args_os());
    let app = api::get_router().layer(middleware::from_fn(print_request));

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

#[cfg(test)]
mod tests {
    #[test]
    fn verify_app() {
        super::build_app().debug_assert()
    }
}
