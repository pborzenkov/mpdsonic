use super::{
    types::{AlbumID, ArtistID, CoverArtID, SongID},
    Error,
};
use crate::mpd::Count;
use axum::{
    extract::{Extension, Query},
    routing::Router,
};
use mpd_client::{
    commands::{Find, List},
    filter::Filter,
    tag::Tag,
};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::log::warn;
use yaserde_derive::YaSerialize;

const ROOT_FOLDER: &str = "/";

pub fn get_router() -> Router {
    Router::new()
        .route("/getMusicFolders.view", super::handler(get_music_folders))
        .route("/getArtists.view", super::handler(get_artists))
        .route("/getArtist.view", super::handler(get_artist))
        .route("/getArtistInfo2.view", super::handler(get_artist_info2))
        .route("/getAlbum.view", super::handler(get_album))
}

async fn get_music_folders() -> super::Result<GetMusicFolders> {
    Ok(GetMusicFolders {
        music_folders: vec![MusicFolder {
            id: ROOT_FOLDER.to_string(),
            name: "Music".to_string(),
        }],
    })
}

#[derive(Serialize, YaSerialize)]
struct MusicFolder {
    #[yaserde(attribute)]
    id: String,
    #[yaserde(attribute)]
    name: String,
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "musicFolders")]
struct GetMusicFolders {
    #[yaserde(child, rename = "musicFolder")]
    #[serde(rename = "musicFolder")]
    music_folders: Vec<MusicFolder>,
}

impl super::Reply for GetMusicFolders {
    fn field_name() -> Option<&'static str> {
        Some("musicFolders")
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArtistsQuery {
    music_folder_id: Option<String>,
}

async fn get_artists(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<GetArtistsQuery>,
) -> super::Result<GetArtists> {
    match param.music_folder_id.as_deref() {
        Some(ROOT_FOLDER) | None => (),
        _ => return Err(Error::generic_error(None)),
    };

    let reply = state
        .client
        .command(List::new(Tag::AlbumSort).group_by(Tag::ArtistSort))
        .await?;

    let index = reply
        .fields
        .iter()
        .fold(vec![], |mut artists: Vec<Artist>, (tag, value)| {
            match tag {
                Tag::ArtistSort => artists.push(Artist {
                    id: ArtistID::new(&value),
                    name: value.clone(),
                    album_count: 0,
                }),
                Tag::AlbumSort => {
                    if let Some(a) = artists.last_mut() {
                        a.album_count += 1;
                    }
                }
                _ => (),
            };

            artists
        })
        .into_iter()
        .fold(vec![], |mut index: Vec<Index>, a| {
            let idx_name = match a.name.chars().next() {
                Some(c) => c.to_uppercase().to_string(),
                _ => return index,
            };

            match index.last_mut() {
                Some(idx) if idx.name == idx_name => idx.artists.push(a),
                _ => index.push(Index {
                    name: idx_name,
                    artists: vec![a],
                }),
            };

            index
        });

    Ok(GetArtists { index })
}

#[derive(Serialize, YaSerialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Artist {
    #[yaserde(attribute)]
    id: ArtistID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute, rename = "albumCount")]
    album_count: u32,
}

#[derive(Serialize, YaSerialize, Debug)]
struct Index {
    #[yaserde(attribute)]
    name: String,
    #[yaserde(child, rename = "artist")]
    #[serde(rename = "artist")]
    artists: Vec<Artist>,
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "artists")]
struct GetArtists {
    index: Vec<Index>,
}

impl super::Reply for GetArtists {
    fn field_name() -> Option<&'static str> {
        Some("artists")
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArtistQuery {
    #[serde(rename = "id")]
    artist: ArtistID,
}

async fn get_artist(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<GetArtistQuery>,
) -> super::Result<GetArtist> {
    let reply = state
        .client
        .command(
            Count::new(Filter::tag(Tag::Artist, param.artist.name.clone()))
                .group_by(Tag::AlbumSort),
        )
        .await?;

    let mut albums = reply
        .fields
        .iter()
        .fold(vec![], |mut albums: Vec<Album>, (tag, value)| {
            match &tag {
                Tag::AlbumSort => albums.push(Album {
                    id: AlbumID::new(&value, &param.artist.name),
                    name: value.clone(),
                    artist: param.artist.name.clone(),
                    artist_id: param.artist.clone(),
                    ..Default::default()
                }),
                Tag::Other(t) if t.as_ref() == "songs" => {
                    if let Some(a) = albums.last_mut() {
                        a.song_count = value.parse().unwrap_or(0);
                    }
                }
                Tag::Other(t) if t.as_ref() == "playtime" => {
                    if let Some(a) = albums.last_mut() {
                        a.duration = value.parse::<f32>().map(|d| d as u64).unwrap_or(0);
                    }
                }
                _ => (),
            };

            albums
        });

    let songs = albums
        .iter()
        .map(|a| {
            let filter = Filter::tag(Tag::Artist, a.artist.clone())
                .and(Filter::tag(Tag::Album, a.name.clone()));

            Find::new(filter.clone()).window(0..1)
        })
        .collect::<Vec<_>>();
    let reply = state.client.command_list(songs).await?;

    for (album, songs) in albums.iter_mut().zip(reply) {
        if let Some(song) = songs.first() {
            album.year = song
                .tags
                .get(&Tag::Date)
                .and_then(|v| v.first().and_then(|d| d.parse().ok()));
            album.genre = song.tags.get(&Tag::Genre).map(|v| v.join(", "));
            album.cover_art = CoverArtID::new(&song.file_path().display().to_string());
        }
    }

    Ok(GetArtist {
        id: param.artist.clone(),
        name: param.artist.name.clone(),
        album_count: albums.len(),
        albums,
    })
}

#[derive(Serialize, YaSerialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Album {
    #[yaserde(attribute)]
    id: AlbumID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute)]
    artist: String,
    #[yaserde(attribute, rename = "artistId")]
    artist_id: ArtistID,
    #[yaserde(attribute, rename = "songCount")]
    song_count: u32,
    #[yaserde(attribute)]
    duration: u64,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<u32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<String>,
    #[yaserde(attribute, rename = "coverArt")]
    cover_art: CoverArtID,
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "artist")]
#[serde(rename_all = "camelCase")]
struct GetArtist {
    #[yaserde(attribute)]
    id: ArtistID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute, rename = "albumCount")]
    album_count: usize,
    #[yaserde(child, rename = "album")]
    #[serde(rename = "album")]
    albums: Vec<Album>,
}

impl super::Reply for GetArtist {
    fn field_name() -> Option<&'static str> {
        Some("artist")
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArtistInfo2Query {
    #[serde(rename = "id")]
    artist: ArtistID,
}

async fn get_artist_info2(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<GetArtistInfo2Query>,
) -> super::Result<ArtistInfo2> {
    let reply = state
        .client
        .command(
            List::new(Tag::MusicBrainzArtistId)
                .filter(Filter::tag(Tag::Artist, param.artist.name.clone())),
        )
        .await?;
    if reply.fields.len() > 1 {
        warn!(
            "more than one MusicBrainzArtistID tag for {}, using the first",
            param.artist.name
        );
    }

    // TODO: artwork, similar artists
    Ok(ArtistInfo2 {
        music_brainz_id: reply.fields.first().map(|x| x.1.clone()),
    })
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "artistInfo2")]
#[serde(rename_all = "camelCase")]
struct ArtistInfo2 {
    #[yaserde(child, rename = "musicBrainzId")]
    music_brainz_id: Option<String>,
}

impl super::Reply for ArtistInfo2 {
    fn field_name() -> Option<&'static str> {
        Some("artistInfo2")
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAlbumQuery {
    #[serde(rename = "id")]
    album: AlbumID,
}

async fn get_album(
    Extension(state): Extension<Arc<super::State>>,
    Query(param): Query<GetAlbumQuery>,
) -> super::Result<GetAlbum> {
    let reply_songs = state
        .client
        .command(Find::new(
            Filter::tag(Tag::Artist, param.album.artist.clone())
                .and(Filter::tag(Tag::Album, param.album.name.clone())),
        ))
        .await?;
    let reply_count = state
        .client
        .command(Count::new(
            Filter::tag(Tag::Artist, param.album.artist.clone())
                .and(Filter::tag(Tag::Album, param.album.name.clone())),
        ))
        .await?;

    let mut resp = GetAlbum {
        id: param.album.clone(),
        name: param.album.name.clone(),
        artist: param.album.artist.clone(),
        artist_id: ArtistID::new(&param.album.artist),
        year: reply_songs.first().and_then(|s| {
            s.tags
                .get(&Tag::Date)
                .and_then(|v| v.first().and_then(|d| d.parse().ok()))
        }),
        genre: reply_songs
            .first()
            .and_then(|s| s.tags.get(&Tag::Genre).map(|v| v.join(", "))),
        cover_art: reply_songs
            .first()
            .map(|s| CoverArtID::new(&s.file_path().display().to_string()))
            .unwrap_or_default(),
        songs: reply_songs
            .into_iter()
            .map(|s| Song {
                id: SongID::new(&s.file_path().display().to_string()),
                title: s.title().map(str::to_string),
                album: param.album.name.clone(),
                artist: param.album.artist.clone(),
                track: s
                    .tags
                    .get(&Tag::Track)
                    .and_then(|v| v.first().and_then(|v| v.parse().ok())),
                year: s
                    .tags
                    .get(&Tag::Date)
                    .and_then(|v| v.first().and_then(|v| v.parse().ok())),
                genre: s.tags.get(&Tag::Genre).map(|v| v.join(", ")),
                cover_art: CoverArtID::new(&s.file_path().display().to_string()),
                duration: s.duration.map(|v| v.as_secs()),
                path: s.file_path().display().to_string(),
                album_id: param.album.clone(),
                artist_id: ArtistID::new(&param.album.artist),
            })
            .collect(),
        ..Default::default()
    };
    for (tag, value) in reply_count.fields {
        match tag {
            Tag::Other(tag) if tag.as_ref() == "songs" => {
                resp.song_count = value.parse().unwrap_or(0)
            }
            Tag::Other(tag) if tag.as_ref() == "playtime" => {
                resp.duration = value.parse::<f32>().map(|d| d as u64).unwrap_or(0)
            }
            _ => (),
        };
    }

    Ok(resp)
}

#[derive(Serialize, YaSerialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct Song {
    #[yaserde(attribute)]
    id: SongID,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[yaserde(attribute)]
    album: String,
    #[yaserde(attribute)]
    artist: String,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    track: Option<u32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<u32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<String>,
    #[yaserde(attribute, rename = "coverArt")]
    cover_art: CoverArtID,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u64>,
    #[yaserde(attribute)]
    path: String,
    #[yaserde(attribute, rename = "albumId")]
    album_id: AlbumID,
    #[yaserde(attribute, rename = "artistId")]
    artist_id: ArtistID,
}

#[derive(Default, Serialize, YaSerialize)]
#[yaserde(rename = "album")]
#[serde(rename_all = "camelCase")]
struct GetAlbum {
    #[yaserde(attribute)]
    id: AlbumID,
    #[yaserde(attribute)]
    name: String,
    #[yaserde(attribute)]
    artist: String,
    #[yaserde(attribute, rename = "artistId")]
    artist_id: ArtistID,
    #[yaserde(attribute, rename = "songCount")]
    song_count: u32,
    #[yaserde(attribute)]
    duration: u64,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<u32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<String>,
    #[yaserde(attribute, rename = "coverArt")]
    cover_art: CoverArtID,
    #[yaserde(child, rename = "song")]
    #[serde(rename = "song")]
    songs: Vec<Song>,
}

impl super::Reply for GetAlbum {
    fn field_name() -> Option<&'static str> {
        Some("album")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Album, Artist, ArtistInfo2, GetAlbum, GetArtist, GetArtists, GetMusicFolders, Index,
        MusicFolder, Song, ROOT_FOLDER,
    };
    use crate::api::{
        expect_ok_json, expect_ok_xml, json,
        types::{AlbumID, ArtistID, CoverArtID, SongID},
        xml,
    };
    use serde_json::json;

    #[test]
    fn get_user() {
        let get_user = GetMusicFolders {
            music_folders: vec![MusicFolder {
                id: ROOT_FOLDER.to_string(),
                name: "Music".to_string(),
            }],
        };
        assert_eq!(
            xml(&get_user),
            expect_ok_xml(Some(
                r#"<musicFolders>
    <musicFolder id="/" name="Music" />
  </musicFolders>"#
            ),)
        );

        assert_eq!(
            json(&get_user),
            expect_ok_json(Some(json!({"musicFolders": {
                "musicFolder": [
                    {
                        "id": ROOT_FOLDER,
                        "name": "Music",
                    }
                ]
                }
            })),),
        );
    }

    #[test]
    fn get_artists() {
        let get_artists = GetArtists {
            index: vec![
                Index {
                    name: "A".to_string(),
                    artists: vec![Artist {
                        id: ArtistID::new("alpha"),
                        name: "alpha".to_string(),
                        album_count: 2,
                    }],
                },
                Index {
                    name: "M".to_string(),
                    artists: vec![
                        Artist {
                            id: ArtistID::new("moo1"),
                            name: "Moo".to_string(),
                            album_count: 1,
                        },
                        Artist {
                            id: ArtistID::new("moo2"),
                            name: "Moo2".to_string(),
                            album_count: 3,
                        },
                    ],
                },
            ],
        };
        assert_eq!(
            xml(&get_artists),
            expect_ok_xml(Some(
                r#"<artists>
    <index name="A">
      <artist id="eyJuYW1lIjoiYWxwaGEifQ==" name="alpha" albumCount="2" />
    </index>
    <index name="M">
      <artist id="eyJuYW1lIjoibW9vMSJ9" name="Moo" albumCount="1" />
      <artist id="eyJuYW1lIjoibW9vMiJ9" name="Moo2" albumCount="3" />
    </index>
  </artists>"#
            ),)
        );

        assert_eq!(
            json(&get_artists),
            expect_ok_json(Some(json!({"artists": {
                "index": [
                    {
                        "name": "A",
                        "artist": [
                            {
                                "id": "eyJuYW1lIjoiYWxwaGEifQ==",
                                "name": "alpha",
                                "albumCount": 2,
                            }
                        ]
                    },
                    {
                        "name": "M",
                        "artist": [
                            {
                                "id": "eyJuYW1lIjoibW9vMSJ9",
                                "name": "Moo",
                                "albumCount": 1,
                            },
                            {
                                "id": "eyJuYW1lIjoibW9vMiJ9",
                                "name": "Moo2",
                                "albumCount": 3,
                            }
                        ]
                    }
                ]
            }
            })),),
        );
    }

    #[test]
    fn get_artist() {
        let get_artist = GetArtist {
            id: ArtistID::new("alpha"),
            name: "alpha".to_string(),
            album_count: 2,
            albums: vec![
                Album {
                    id: AlbumID::new("album1", "alpha"),
                    name: "album1".to_string(),
                    artist: "alpha".to_string(),
                    artist_id: ArtistID::new("alpha"),
                    song_count: 10,
                    duration: 300,
                    year: Some(2000),
                    genre: Some("rock".to_string()),
                    cover_art: CoverArtID::new("artwork1"),
                },
                Album {
                    id: AlbumID::new("album2", "alpha"),
                    name: "album2".to_string(),
                    artist: "alpha".to_string(),
                    artist_id: ArtistID::new("alpha"),
                    song_count: 20,
                    duration: 450,
                    year: None,
                    genre: None,
                    cover_art: CoverArtID::new("artwork2"),
                },
            ],
        };
        assert_eq!(
            xml(&get_artist),
            expect_ok_xml(Some(
                r#"<artist id="eyJuYW1lIjoiYWxwaGEifQ==" name="alpha" albumCount="2">
    <album id="eyJuYW1lIjoiYWxidW0xIiwiYXJ0aXN0IjoiYWxwaGEifQ==" name="album1" artist="alpha" artistId="eyJuYW1lIjoiYWxwaGEifQ==" songCount="10" duration="300" year="2000" genre="rock" coverArt="eyJwYXRoIjoiYXJ0d29yazEifQ==" />
    <album id="eyJuYW1lIjoiYWxidW0yIiwiYXJ0aXN0IjoiYWxwaGEifQ==" name="album2" artist="alpha" artistId="eyJuYW1lIjoiYWxwaGEifQ==" songCount="20" duration="450" coverArt="eyJwYXRoIjoiYXJ0d29yazIifQ==" />
  </artist>"#
            ),)
        );

        assert_eq!(
            json(&get_artist),
            expect_ok_json(Some(json!({"artist": {
                "id": "eyJuYW1lIjoiYWxwaGEifQ==",
                "name": "alpha",
                "albumCount": 2,
                "album": [
                    {
                        "id": "eyJuYW1lIjoiYWxidW0xIiwiYXJ0aXN0IjoiYWxwaGEifQ==",
                        "name": "album1",
                        "artist": "alpha",
                        "artistId": "eyJuYW1lIjoiYWxwaGEifQ==",
                        "songCount": 10,
                        "duration": 300,
                        "year": 2000,
                        "genre": "rock",
                        "coverArt": "eyJwYXRoIjoiYXJ0d29yazEifQ==",
                    },
                    {
                        "id": "eyJuYW1lIjoiYWxidW0yIiwiYXJ0aXN0IjoiYWxwaGEifQ==",
                        "name": "album2",
                        "artist": "alpha",
                        "artistId": "eyJuYW1lIjoiYWxwaGEifQ==",
                        "songCount": 20,
                        "duration": 450,
                        "coverArt": "eyJwYXRoIjoiYXJ0d29yazIifQ==",
                    },
                ]
            }
            })),),
        );
    }

    #[test]
    fn get_artist_info2() {
        let get_artist_info2 = ArtistInfo2 {
            music_brainz_id: Some("788ad31c-bf0c-4a31-83f8-b8b130d79c76".to_string()),
        };
        assert_eq!(
            xml(&get_artist_info2),
            expect_ok_xml(Some(
                r#"<artistInfo2>
    <musicBrainzId>788ad31c-bf0c-4a31-83f8-b8b130d79c76</musicBrainzId>
  </artistInfo2>"#
            ),)
        );

        assert_eq!(
            json(&get_artist_info2),
            expect_ok_json(Some(json!({"artistInfo2": {
                "musicBrainzId": "788ad31c-bf0c-4a31-83f8-b8b130d79c76",
            }
            })),),
        );
    }

    #[test]
    fn get_album() {
        let get_album = GetAlbum {
            id: AlbumID::new("alpha", "beta"),
            name: "beta".to_string(),
            artist: "alpha".to_string(),
            artist_id: ArtistID::new("alpha"),
            song_count: 2,
            duration: 300,
            year: Some(2020),
            genre: Some("rock".to_string()),
            cover_art: CoverArtID::new("artwork"),
            songs: vec![
                Song {
                    id: SongID::new("song1"),
                    title: Some("song1".to_string()),
                    album: "beta".to_string(),
                    artist: "alpha".to_string(),
                    track: Some(1),
                    year: Some(2020),
                    genre: Some("rock".to_string()),
                    cover_art: CoverArtID::new("artwork"),
                    duration: Some(300),
                    path: "path1".to_string(),
                    album_id: AlbumID::new("alpha", "beta"),
                    artist_id: ArtistID::new("alpha"),
                },
                Song {
                    id: SongID::new("song2"),
                    title: None,
                    album: "beta".to_string(),
                    artist: "alpha".to_string(),
                    track: None,
                    year: None,
                    genre: None,
                    cover_art: CoverArtID::new("artwork"),
                    duration: None,
                    path: "path2".to_string(),
                    album_id: AlbumID::new("alpha", "beta"),
                    artist_id: ArtistID::new("alpha"),
                },
            ],
        };
        assert_eq!(
            xml(&get_album),
            expect_ok_xml(Some(
                r#"<album id="eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=" name="beta" artist="alpha" artistId="eyJuYW1lIjoiYWxwaGEifQ==" songCount="2" duration="300" year="2020" genre="rock" coverArt="eyJwYXRoIjoiYXJ0d29yayJ9">
    <song id="eyJwYXRoIjoic29uZzEifQ==" title="song1" album="beta" artist="alpha" track="1" year="2020" genre="rock" coverArt="eyJwYXRoIjoiYXJ0d29yayJ9" duration="300" path="path1" albumId="eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=" artistId="eyJuYW1lIjoiYWxwaGEifQ==" />
    <song id="eyJwYXRoIjoic29uZzIifQ==" album="beta" artist="alpha" coverArt="eyJwYXRoIjoiYXJ0d29yayJ9" path="path2" albumId="eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=" artistId="eyJuYW1lIjoiYWxwaGEifQ==" />
  </album>"#
            ),)
        );

        assert_eq!(
            json(&get_album),
            expect_ok_json(Some(json!({"album": {
                "id": "eyJuYW1lIjoiYWxwaGEiLCJhcnRpc3QiOiJiZXRhIn0=",
                "name": "beta",
                "artist": "alpha",
                "artistId": "eyJuYW1lIjoiYWxwaGEifQ==",
                "songCount": 2,
                "duration": 300,
                "year": 2020,
                "genre": "rock",
                "coverArt": "eyJwYXRoIjoiYXJ0d29yayJ9",
                "song": [
                    {
                        "id": "eyJwYXRoIjoic29uZzEifQ==",
                        "title": "song1",
                        "album": "beta",
                        "artist": "alpha",
                        "track": 1,
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
