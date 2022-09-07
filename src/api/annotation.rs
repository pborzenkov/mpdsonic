use super::{types::SongID, Error};
use axum::{extract::Query, routing::Router, Extension};
use mpd_client::{commands::Find, filter::Filter, tag::Tag};
use serde::Deserialize;
use std::sync::Arc;

pub(crate) fn get_router() -> Router {
    Router::new().route("/scrobble.view", super::handler(scrobble))
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScrobbleQuery {
    #[serde(rename = "id")]
    song: SongID,
    time: Option<u64>,
    submission: Option<bool>,
}

async fn scrobble(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<ScrobbleQuery>,
) -> super::Result<()> {
    let listenbrainz = state
        .listenbrainz
        .clone()
        .ok_or_else(|| Error::generic_error(Some("ListenBrainz client is not configured")))?;

    let songs = state
        .client
        .command(Find::new(Filter::tag(
            Tag::Other("file".into()),
            param.song.path,
        )))
        .await?;
    let song = songs.first().ok_or_else(Error::not_found)?;

    match param.submission {
        Some(true) => {
            listenbrainz
                .listen(song, param.time.unwrap_or_default())
                .await?
        }
        _ => listenbrainz.playing_now(song).await?,
    }

    Ok(())
}
