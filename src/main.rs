use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use clap::Parser;
use mpd_client::{commands::SetBinaryLimit, Client};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, warn};

mod api;
mod library;

#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    #[clap(
        short,
        long,
        help = "Address to listen on",
        default_value = "127.0.0.1:3000"
    )]
    address: SocketAddr,
    #[clap(short, long, help = "Subsonic API username", env = "MPDSONIC_USERNAME")]
    username: String,
    #[clap(short, long, help = "Subsonic API password", env = "MPDSONIC_PASSWORD")]
    password: String,
    #[clap(long, help = "MPD address", default_value = "127.0.0.1:6600")]
    mpd_address: SocketAddr,
    #[clap(long, help = "MPD password", env = "MPDSONIC_MPD_PASSWORD")]
    mpd_password: Option<String>,
    #[clap(long, help = "MPD library location")]
    mpd_library: String,
}

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

    let args = Args::parse();

    let connection = TcpStream::connect(args.mpd_address).await.unwrap();
    let (client, _) = Client::connect_with_password_opt(connection, args.mpd_password.as_deref())
        .await
        .unwrap();

    client.command(SetBinaryLimit(128 * 1024)).await.unwrap();

    let auth = api::Authentication::new(&args.username, &args.password);
    let app = api::get_router(
        auth,
        client,
        library::get_library(&args.mpd_library).unwrap(),
    )
    .layer(middleware::from_fn(print_request));

    axum::Server::bind(&args.address)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
