use crate::listenbrainz;

use super::{common::STICKER_RATING, types::SongID, Error};
use axum::{extract::Query, routing::Router, Extension};
use mpd_client::{
    commands::{Find, StickerDelete, StickerSet},
    filter::Filter,
    tag::Tag,
};
use serde::Deserialize;
use std::sync::Arc;
use time::OffsetDateTime;

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/scrobble.view", super::handler(scrobble))
        .route("/setRating.view", super::handler(set_rating))
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScrobbleQuery {
    #[serde(rename = "id")]
    song: SongID,
    time: Option<i64>,
    submission: Option<bool>,
}

async fn scrobble(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<ScrobbleQuery>,
) -> super::Result<()> {
    let listenbrainz = state
        .listenbrainz
        .as_ref()
        .ok_or_else(|| Error::generic_error(Some("ListenBrainz client is not configured")))?;

    let songs = state
        .pool
        .get()
        .await?
        .command(Find::new(Filter::tag(
            Tag::Other("file".into()),
            param.song.path,
        )))
        .await?;
    let song = songs.first().ok_or_else(Error::not_found)?;

    match param.submission {
        Some(true) => {
            listenbrainz
                .listen(
                    song,
                    param
                        .time
                        .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp()),
                )
                .await?
        }
        _ => listenbrainz.playing_now(song).await?,
    }

    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetRatingQuery {
    #[serde(rename = "id")]
    song: SongID,
    rating: u8,
}

async fn set_rating(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<SetRatingQuery>,
) -> super::Result<()> {
    let pool = state.pool.get().await?;
    if param.rating > 0 {
        pool.command(StickerSet::new(
            &param.song.path,
            STICKER_RATING,
            &param.rating.to_string(),
        ))
        .await?;
    } else {
        pool.command(StickerDelete::new(&param.song.path, "rating"))
            .await?;
    };

    let listenbrainz = if let Some(ref client) = state.listenbrainz {
        client
    } else {
        return Ok(());
    };

    let songs = pool
        .command(Find::new(Filter::tag(
            Tag::Other("file".into()),
            &param.song.path,
        )))
        .await?;
    let song = songs.first().ok_or_else(Error::not_found)?;

    if let Some(score) = match param.rating {
        0 => Some(listenbrainz::Score::Remove),
        1 => Some(listenbrainz::Score::Hate),
        5 => Some(listenbrainz::Score::Love),
        _ => None,
    } {
        listenbrainz.feedback(song, score).await?;
    }

    Ok(())
}
