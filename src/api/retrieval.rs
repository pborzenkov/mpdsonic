use super::{error::Error, types::CoverArtID};
use axum::{
    extract::{Extension, Query},
    response::{IntoResponse, Response},
    routing::Router,
};
use bytes::{BufMut, BytesMut};
use http::header::{self, HeaderValue};
use mpd_client::commands::definitions::AlbumArt;
use serde::Deserialize;
use std::sync::Arc;

pub fn get_router() -> Router {
    Router::new().route("/getCoverArt.view", super::raw_handler(get_cover_art))
}

#[derive(Clone, Deserialize)]
struct GetCoverArtQuery {
    #[serde(rename = "id")]
    song: CoverArtID,
}

async fn get_cover_art(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<GetCoverArtQuery>,
) -> super::Result<Response> {
    let mut cover = BytesMut::new();

    loop {
        let resp = state
            .client
            .command(AlbumArt::new(params.song.path.clone()).offset(cover.len()))
            .await?
            .ok_or(Error::not_found())?;

        cover.put_slice(resp.data());
        if cover.len() < resp.size {
            continue;
        }

        let mut res = cover.into_response();
        if let Some(m) = resp.mime.and_then(|m| HeaderValue::from_str(&m).ok()) {
            res.headers_mut().insert(header::CONTENT_TYPE, m);
        }

        return Ok(res);
    }
}
