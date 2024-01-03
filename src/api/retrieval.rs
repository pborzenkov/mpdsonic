use super::{
    error::Error,
    types::{CoverArtID, SongID},
};
use crate::library;
use axum::{
    body::StreamBody,
    extract::{Extension, Query},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    routing::Router,
};
use bytes::{BufMut, Bytes, BytesMut};
use futures::Stream;
use futures::StreamExt;
use mpd_client::commands::{AlbumArt, GetPlaylist};
use serde::Deserialize;
use std::{pin::Pin, process::Stdio, sync::Arc};
use tokio::process::Command;
use tokio_util::io::{ReaderStream, StreamReader};
use tracing::warn;

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/getCoverArt.view", super::raw_handler(get_cover_art))
        .route("/stream.view", super::raw_handler(stream))
        .route("/getAvatar.view", super::raw_handler(get_avatar))
}

#[derive(Clone, Deserialize)]
struct GetCoverArtQuery {
    #[serde(rename = "id")]
    cover: CoverArtID,
}

async fn get_cover_art(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<GetCoverArtQuery>,
) -> super::Result<Response> {
    let path = match params.cover {
        CoverArtID::Song { path } => path,
        CoverArtID::Playlist { name } => {
            let songs = state.pool.get().await?.command(GetPlaylist(&name)).await?;

            songs
                .first()
                .map(|s| s.file_path().display().to_string())
                .ok_or_else(Error::not_found)?
        }
    };

    let mut cover = BytesMut::new();
    loop {
        let resp = state
            .pool
            .get()
            .await?
            .command(AlbumArt::new(&path).offset(cover.len()))
            .await?
            .ok_or_else(Error::not_found)?;

        cover.put_slice(&resp.data);
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

#[derive(Clone, Deserialize)]
struct StreamQuery {
    #[serde(rename = "id")]
    song: SongID,
    #[serde(rename = "maxBitRate")]
    max_bitrate: Option<u32>,
    format: Option<String>,
}

static FFMPEG_ARGS: &[&str] = &[
                "-v",
                "0",
                "-i",
                "-",
                "-map",
                "0:a:0",
                "-vn",
                "-b:a",
                "<bitrate>",
                "-c:a",
                "libopus",
                "-vbr",
                "on",
                "-af",
                "volume=replaygain=track:replaygain_preamp=6dB:replaygain_noclip=0, alimiter=level=disabled, asidedata=mode=delete:type=REPLAYGAIN",
                "-metadata",
                "replaygain_album_gain=",
                "-metadata",
                "replaygain_album_peak=",
                "-metadata",
                "replaygain_track_gain=",
                "-metadata",
                "replaygain_track_peak=",
                "-metadata",
                "r128_album_gain=",
                "-metadata",
                "r128_track_gain=",
                "-f",
                "opus",
                "-"
    ];
static FFMPEG_BITRATES: &[u32] = &[96, 112, 128, 160, 192];

async fn stream(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<StreamQuery>,
) -> super::Result<StreamBody<Pin<Box<dyn Stream<Item = library::Result<Bytes>> + Send>>>> {
    let input_stream = state.lib.get_song(&params.song.path).await?;

    let output_stream = match params.format.as_deref() {
        Some("raw") => input_stream,
        Some("ogg") | None => {
            let max_available_bitrate = FFMPEG_BITRATES[FFMPEG_BITRATES.len() - 1];
            let max_desired_bitrate = match params.max_bitrate {
                None | Some(0) => max_available_bitrate,
                Some(b) => b,
            };
            let bitrate = FFMPEG_BITRATES
                .get(
                    FFMPEG_BITRATES
                        .partition_point(|&x| x <= max_desired_bitrate)
                        .saturating_sub(1),
                )
                .copied()
                .unwrap_or(max_available_bitrate)
                * 1024;
            let ffmpeg_args = FFMPEG_ARGS
                .iter()
                .map(|&a| {
                    if a == "<bitrate>" {
                        bitrate.to_string()
                    } else {
                        a.to_string()
                    }
                })
                .collect::<Vec<_>>();

            let mut child = Command::new("ffmpeg")
                .args(ffmpeg_args)
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| Error::generic_error(Some("cannot capture child's stdin")))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| Error::generic_error(Some("cannot capture child's stdout")))?;

            tokio::spawn(async move {
                if let Err(err) =
                    tokio::io::copy(&mut StreamReader::new(input_stream), &mut stdin).await
                {
                    warn!(path = ?params.song.path, action = "copy", err = ?err);
                }
                drop(stdin);
                if let Err(err) = child.wait().await {
                    warn!(path = ?params.song.path, action = "wait", err = ?err);
                }
            });

            ReaderStream::new(stdout)
                .map(|x| x.map_err(Into::into))
                .boxed()
        }
        Some(_) => return Err(Error::generic_error(Some("unsupported format"))),
    };

    Ok(StreamBody::new(output_stream))
}

#[derive(Clone, Deserialize)]
struct GetAvatarQuery {
    u: String,
    username: String,
}

async fn get_avatar(Query(params): Query<GetAvatarQuery>) -> super::Result<Response> {
    match params.u == params.username {
        true => Err(Error::not_found()),
        false => Err(Error::not_authorized(&format!(
            "{} is not authorized to get details for other users.",
            params.u
        ))),
    }
}
