use crate::listenbrainz;

use super::{
    common::{STICKER_RATING, STICKER_STARRED},
    types::SongID,
    Error,
};
use axum::{extract::Query, routing::Router, Extension};
use mpd_client::{
    commands::{Find, StickerDelete, StickerSet},
    filter::Filter,
    tag::Tag,
};
use serde::Deserialize;
use std::sync::Arc;
use time::{format_description::well_known, OffsetDateTime};

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/scrobble.view", super::handler(scrobble))
        .route("/setRating.view", super::handler(set_rating))
        .route("/star.view", super::handler(star))
        .route("/unstar.view", super::handler(unstar))
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
    let conn = state.pool.get().await?;
    if param.rating > 0 {
        conn.command(StickerSet::new(
            &param.song.path,
            STICKER_RATING,
            &param.rating.to_string(),
        ))
        .await?;
    } else {
        conn.command(StickerDelete::new(&param.song.path, "rating"))
            .await?;
    };

    let listenbrainz = if let Some(ref client) = state.listenbrainz {
        client
    } else {
        return Ok(());
    };

    let songs = conn
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StarQuery {
    #[serde(rename = "id")]
    song: SongID,
}

async fn star(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<StarQuery>,
) -> super::Result<()> {
    state
        .pool
        .get()
        .await?
        .command(StickerSet::new(
            &param.song.path,
            STICKER_STARRED,
            &OffsetDateTime::now_utc()
                .format(&well_known::Rfc3339)
                .map_err(|_| super::Error::generic_error(None))?,
        ))
        .await?;

    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnstarQuery {
    #[serde(rename = "id")]
    song: SongID,
}

async fn unstar(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<UnstarQuery>,
) -> super::Result<()> {
    state
        .pool
        .get()
        .await?
        .command(StickerDelete::new(&param.song.path, STICKER_STARRED))
        .await?;

    Ok(())
}
