use super::{
    common::mpd_song_to_subsonic,
    glue::RawQuery,
    types::{PlaylistID, Song, SongID},
};
use crate::api::error::Error;
use axum::{
    extract::{Extension, Query},
    routing::Router,
};
use mpd_client::commands::{
    self, AddToPlaylist, DeletePlaylist, RemoveFromPlaylist, RenamePlaylist, SaveQueueAsPlaylist,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yaserde_derive::YaSerialize;

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/getPlaylists.view", super::handler(get_playlists))
        .route("/getPlaylist.view", super::handler(get_playlist))
        .route("/createPlaylist.view", super::handler(create_playlist))
        .route("/updatePlaylist.view", super::handler(update_playlist))
        .route("/deletePlaylist.view", super::handler(delete_playlist))
}

#[derive(Clone, Deserialize)]
struct GetPlaylistsQuery {
    u: String,
    username: Option<String>,
}

async fn get_playlists(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<GetPlaylistsQuery>,
) -> super::Result<GetPlaylists> {
    if params.u != params.username.unwrap_or_else(|| params.u.clone()) {
        return Err(super::Error::not_authorized(&format!(
            "{} is not authorized to get details for other users.",
            params.u
        )));
    }

    let playlists = state
        .pool
        .get()
        .await?
        .command(commands::GetPlaylists)
        .await?;
    let playlists_songs = state
        .pool
        .get()
        .await?
        .command_list(
            playlists
                .iter()
                .map(|p| commands::GetPlaylist(&p.name))
                .collect::<Vec<_>>(),
        )
        .await?;

    Ok(GetPlaylists {
        playlists: playlists
            .iter()
            .zip(playlists_songs)
            .map(|(p, songs)| Playlist {
                id: PlaylistID::new(&p.name),
                name: p.name.clone(),
                owner: params.u.clone(),
                public: true,
                song_count: songs.len(),
                duration: songs
                    .iter()
                    .map(|s| s.duration.map(|v| v.as_secs()).unwrap_or(0))
                    .sum(),
                changed: p.last_modified.chrono_datetime().to_rfc3339(),
            })
            .collect(),
    })
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "playlists")]
struct GetPlaylists {
    #[yaserde(child, rename = "playlist")]
    #[serde(rename = "playlist")]
    playlists: Vec<Playlist>,
}

impl super::Reply for GetPlaylists {
    fn field_name() -> Option<&'static str> {
        Some("playlists")
    }
}

#[derive(Serialize, YaSerialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Playlist {
    #[yaserde(attribute)]
    id: PlaylistID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute)]
    owner: String,
    #[yaserde(attribute)]
    public: bool,
    #[yaserde(attribute, rename = "songCount")]
    song_count: usize,
    #[yaserde(attribute)]
    duration: u64,
    #[yaserde(attribute)]
    changed: String,
}

#[derive(Clone, Deserialize)]
struct GetPlaylistQuery {
    u: String,
    #[serde(rename = "id")]
    playlist: PlaylistID,
}

async fn get_playlist(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<GetPlaylistQuery>,
) -> super::Result<GetPlaylist> {
    let (playlists, songs) = state
        .pool
        .get()
        .await?
        .command_list((
            commands::GetPlaylists,
            commands::GetPlaylist(&params.playlist.name),
        ))
        .await?;

    Ok(GetPlaylist {
        id: params.playlist.clone(),
        name: params.playlist.name.clone(),
        owner: params.u.clone(),
        public: true,
        song_count: songs.len(),
        duration: songs
            .iter()
            .map(|s| s.duration.map(|v| v.as_secs()).unwrap_or(0))
            .sum(),
        changed: playlists
            .iter()
            .find(|&p| p.name == params.playlist.name)
            .map(|p| p.last_modified.chrono_datetime().to_rfc3339()),
        songs: songs.into_iter().map(mpd_song_to_subsonic).collect(),
    })
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "playlist")]
#[serde(rename_all = "camelCase")]
struct GetPlaylist {
    #[yaserde(attribute)]
    id: PlaylistID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute)]
    owner: String,
    #[yaserde(attribute)]
    public: bool,
    #[yaserde(attribute, rename = "songCount")]
    song_count: usize,
    #[yaserde(attribute)]
    duration: u64,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    changed: Option<String>,
    #[yaserde(child, rename = "entry")]
    #[serde(rename = "entry")]
    songs: Vec<Song>,
}

impl super::Reply for GetPlaylist {
    fn field_name() -> Option<&'static str> {
        Some("playlist")
    }
}

#[derive(Clone, Deserialize, Debug)]
struct CreatePlaylistQuery {
    u: String,
    #[serde(rename = "name")]
    playlist: String,
}

async fn create_playlist(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<CreatePlaylistQuery>,
    RawQuery(query): RawQuery,
) -> super::Result<GetPlaylist> {
    let songs = url::form_urlencoded::parse(
        &query
            .ok_or_else(|| Error::missing_parameter("failed to parse URL query"))?
            .into_bytes(),
    )
    .filter_map(|(k, v)| match k.as_ref() {
        "songId" => SongID::try_from(v.as_ref()).ok(),
        _ => None,
    })
    .collect::<Vec<_>>();
    if songs.is_empty() {
        return Err(Error::missing_parameter("songId is missing"));
    }

    let conn = state.pool.get().await?;

    conn.command(SaveQueueAsPlaylist(&params.playlist)).await?;
    conn.command(RemoveFromPlaylist::range(&params.playlist, ..))
        .await?;
    conn.command_list(
        songs
            .iter()
            .map(|s| AddToPlaylist::new(&params.playlist, &s.path))
            .collect::<Vec<_>>(),
    )
    .await?;
    drop(conn);

    get_playlist(
        Extension(state),
        Query(GetPlaylistQuery {
            u: params.u,
            playlist: PlaylistID::new(&params.playlist),
        }),
    )
    .await
}

#[derive(Clone, Deserialize, Debug)]
struct UpdatePlaylistQuery {
    #[serde(rename = "playlistId")]
    playlist: PlaylistID,
    name: Option<String>,
}

async fn update_playlist(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<UpdatePlaylistQuery>,
    RawQuery(query): RawQuery,
) -> super::Result<()> {
    let query = query
        .ok_or_else(|| Error::missing_parameter("failed to parse URL query"))?
        .into_bytes();
    let query = url::form_urlencoded::parse(&query);
    let to_add = query
        .filter_map(|(k, v)| match k.as_ref() {
            "songIdToAdd" => SongID::try_from(v.as_ref()).ok(),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut to_remove = query
        .filter_map(|(k, v)| match k.as_ref() {
            "songIndexToRemove" => v.parse::<usize>().ok(),
            _ => None,
        })
        .collect::<Vec<_>>();
    // reverse sort to make sure that song indicies are always valid during removal
    to_remove.sort_by(|a, b| b.cmp(a));

    let conn = state.pool.get().await?;
    if !to_remove.is_empty() {
        conn.command_list(
            to_remove
                .iter()
                .map(|&idx| RemoveFromPlaylist::position(&params.playlist.name, idx))
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    if !to_add.is_empty() {
        conn.command_list(
            to_add
                .iter()
                .map(|s| AddToPlaylist::new(&params.playlist.name, &s.path))
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    if let Some(name) = params.name {
        conn.command(RenamePlaylist::new(&params.playlist.name, &name))
            .await?;
    };

    Ok(())
}

#[derive(Clone, Deserialize, Debug)]
struct DeletePlaylistQuery {
    #[serde(rename = "id")]
    playlist: PlaylistID,
}

async fn delete_playlist(
    Extension(state): Extension<Arc<super::State>>,
    Query(params): Query<DeletePlaylistQuery>,
) -> super::Result<()> {
    state
        .pool
        .get()
        .await?
        .command(DeletePlaylist(&params.playlist.name))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GetPlaylists, Playlist};
    use crate::api::{
        expect_ok_json, expect_ok_xml, json,
        playlists::GetPlaylist,
        types::{AlbumID, ArtistID, CoverArtID, PlaylistID, Song, SongID},
        xml,
    };
    use serde_json::json;

    #[test]
    fn get_playlists() {
        let get_playlists = GetPlaylists {
            playlists: vec![
                Playlist {
                    id: PlaylistID::new("metal"),
                    name: "metal".to_string(),
                    owner: "me".to_string(),
                    public: true,
                    song_count: 10,
                    duration: 1234,
                    changed: "2022-07-11T10:19:57.652Z".to_string(),
                },
                Playlist {
                    id: PlaylistID::new("rock"),
                    name: "rock".to_string(),
                    owner: "me".to_string(),
                    public: true,
                    song_count: 16,
                    duration: 5678,
                    changed: "2021-06-10T10:19:57.652Z".to_string(),
                },
            ],
        };
        assert_eq!(
            xml(&get_playlists),
            expect_ok_xml(Some(
                r#"<playlists>
    <playlist id="eyJuYW1lIjoibWV0YWwifQ==" name="metal" owner="me" public="true" songCount="10" duration="1234" changed="2022-07-11T10:19:57.652Z" />
    <playlist id="eyJuYW1lIjoicm9jayJ9" name="rock" owner="me" public="true" songCount="16" duration="5678" changed="2021-06-10T10:19:57.652Z" />
  </playlists>"#
            ),)
        );

        assert_eq!(
            json(&get_playlists),
            expect_ok_json(Some(json!({"playlists": {
                "playlist": [
                    {
                        "id": "eyJuYW1lIjoibWV0YWwifQ==",
                        "name": "metal",
                        "owner": "me",
                        "public": true,
                        "songCount": 10,
                        "duration": 1234,
                        "changed": "2022-07-11T10:19:57.652Z",
                    },
                    {
                        "id": "eyJuYW1lIjoicm9jayJ9",
                        "name": "rock",
                        "owner": "me",
                        "public": true,
                        "songCount": 16,
                        "duration": 5678,
                        "changed": "2021-06-10T10:19:57.652Z",
                    }
                ]
            }
            })),),
        );
    }

    #[test]
    fn get_playlist() {
        let get_playlist = GetPlaylist {
            id: PlaylistID::new("metal"),
            name: "metal".to_string(),
            owner: "me".to_string(),
            public: true,
            song_count: 10,
            duration: 1234,
            changed: Some("2022-07-11T10:19:57.652Z".to_string()),
            songs: vec![
                Song {
                    id: SongID::new("song1"),
                    title: Some("song1".to_string()),
                    album: Some("beta".to_string()),
                    artist: "alpha".to_string(),
                    track: Some(1),
                    disc_number: Some(1),
                    year: Some(2020),
                    genre: Some("rock".to_string()),
                    cover_art: CoverArtID::new("artwork"),
                    duration: Some(300),
                    path: "path1".to_string(),
                    album_id: Some(AlbumID::new("alpha", "beta")),
                    artist_id: ArtistID::new("alpha"),
                },
                Song {
                    id: SongID::new("song2"),
                    album: Some("beta".to_string()),
                    artist: "alpha".to_string(),
                    cover_art: CoverArtID::new("artwork"),
                    path: "path2".to_string(),
                    album_id: Some(AlbumID::new("alpha", "beta")),
                    artist_id: ArtistID::new("alpha"),
                    ..Default::default()
                },
            ],
        };
        assert_eq!(
            xml(&get_playlist),
            expect_ok_xml(Some(
                r#"<playlist id="eyJuYW1lIjoibWV0YWwifQ==" name="metal" owner="me" public="true" songCount="10" duration="1234" changed="2022-07-11T10:19:57.652Z">
    <entry id="eyJwYXRoIjoic29uZzEifQ==" title="song1" album="beta" artist="alpha" track="1" discNumber="1" year="2020" genre="rock" coverArt="eyJwYXRoIjoiYXJ0d29yayJ9" duration="300" path="path1" albumId="eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=" artistId="eyJuYW1lIjoiYWxwaGEifQ==" />
    <entry id="eyJwYXRoIjoic29uZzIifQ==" album="beta" artist="alpha" coverArt="eyJwYXRoIjoiYXJ0d29yayJ9" path="path2" albumId="eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=" artistId="eyJuYW1lIjoiYWxwaGEifQ==" />
  </playlist>"#
            ),)
        );

        assert_eq!(
            json(&get_playlist),
            expect_ok_json(Some(json!({"playlist": {
                "id": "eyJuYW1lIjoibWV0YWwifQ==",
                "name": "metal",
                "owner": "me",
                "public": true,
                "songCount": 10,
                "duration": 1234,
                "changed": "2022-07-11T10:19:57.652Z",
                "entry": [
                    {
                        "id": "eyJwYXRoIjoic29uZzEifQ==",
                        "title": "song1",
                        "album": "beta",
                        "artist": "alpha",
                        "track": 1,
                        "discNumber": 1,
                        "year": 2020,
                        "genre": "rock",
                        "coverArt": "eyJwYXRoIjoiYXJ0d29yayJ9",
                        "duration": 300,
                        "path": "path1",
                        "albumId": "eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=",
                        "artistId": "eyJuYW1lIjoiYWxwaGEifQ==",
                    },
                    {
                        "id": "eyJwYXRoIjoic29uZzIifQ==",
                        "album": "beta",
                        "artist": "alpha",
                        "coverArt": "eyJwYXRoIjoiYXJ0d29yayJ9",
                        "path": "path2",
                        "albumId": "eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=",
                        "artistId": "eyJuYW1lIjoiYWxwaGEifQ==",
                    },
                ]
            }
            })),),
        );
    }
}
