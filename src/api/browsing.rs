use axum::{
    extract::{Extension, Query},
    routing::Router,
};
use mpd_client::{commands::definitions::List, tag::Tag};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yaserde_derive::YaSerialize;

const ROOT_FOLDER: &str = "/";

pub fn get_router() -> Router {
    Router::new()
        .route("/getMusicFolders.view", super::handler(get_music_folders))
        .route("/getArtists.view", super::handler(get_artists))
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
    Query(param): Query<GetArtistsQuery>,
    Extension(state): Extension<Arc<super::State>>,
) -> super::Result<GetArtists> {
    match param.music_folder_id.as_deref() {
        Some(ROOT_FOLDER) | None => (),
        _ => return Err(super::Error::generic_error()),
    };

    let reply = state
        .client
        .command(List::new(Tag::AlbumSort).group_by(Tag::ArtistSort))
        .await?;

    let index = reply
        .fields
        .iter()
        .fold(vec![], |mut artists: Vec<Artist>, f| {
            match f.0 {
                Tag::ArtistSort => artists.push(Artist {
                    id: f.1.clone(),
                    name: f.1.clone(),
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
            let idx_name = match a.id.chars().next() {
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
    id: String,
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

#[cfg(test)]
mod tests {
    use super::{Artist, GetArtists, GetMusicFolders, Index, MusicFolder, ROOT_FOLDER};
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};
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
                        id: "alpha".to_string(),
                        name: "alpha".to_string(),
                        album_count: 2,
                    }],
                },
                Index {
                    name: "M".to_string(),
                    artists: vec![
                        Artist {
                            id: "moo1".to_string(),
                            name: "Moo".to_string(),
                            album_count: 1,
                        },
                        Artist {
                            id: "moo2".to_string(),
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
      <artist id="alpha" name="alpha" albumCount="2" />
    </index>
    <index name="M">
      <artist id="moo1" name="Moo" albumCount="1" />
      <artist id="moo2" name="Moo2" albumCount="3" />
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
                                "id": "alpha",
                                "name": "alpha",
                                "albumCount": 2,
                            }
                        ]
                    },
                    {
                        "name": "M",
                        "artist": [
                            {
                                "id": "moo1",
                                "name": "Moo",
                                "albumCount": 1,
                            },
                            {
                                "id": "moo2",
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
}
