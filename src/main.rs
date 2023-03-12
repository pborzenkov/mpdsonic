use axum::{
    self,
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
};
use clap::Parser;
use std::{net::SocketAddr, time::Duration};
use tracing::{debug, warn};

mod api;
mod library;
mod listenbrainz;
mod mpd;

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
    #[clap(long, help = "ListenBrainz token", env = "MPDSONIC_LISTENBRAINZ_TOKEN")]
    listenbrainz_token: Option<String>,
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

    if let Err(err) = run_main().await {
        eprintln!("ERROR: {}", err);
        std::process::exit(-1);
    }
}

async fn run_main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let manager = mpd::ConnectionManager::new(&args.mpd_address, &args.mpd_password);
    let pool = bb8::Pool::builder()
        .max_size(8)
        .connection_timeout(Duration::from_secs(1))
        .connection_customizer(Box::new(mpd::ConnectionCustomizer))
        .build(manager)
        .await?;

    let auth = api::Authentication::new(&args.username, &args.password);
    let app = api::get_router(
        auth,
        pool,
        library::get_library(&args.mpd_library).await?,
        args.listenbrainz_token
            .and_then(|t| listenbrainz::Client::new(&t).ok()),
    )
    .layer(middleware::from_fn(print_request));

    axum::Server::bind(&args.address)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
