use super::types::PlaylistID;
use axum::{
    extract::{Extension, Query},
    routing::Router,
};
use mpd_client::commands::{self, GetPlaylist};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yaserde_derive::YaSerialize;

pub fn get_router() -> Router {
    Router::new().route("/getPlaylists.view", super::handler(get_playlists))
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
    if params.u != params.username.unwrap_or(params.u.clone()) {
        return Err(super::Error::not_authorized(&format!(
            "{} is not authorized to get details for other users.",
            params.u
        )));
    }

    let playlists = state.client.command(commands::GetPlaylists).await?;
    let playlists_info = state
        .client
        .command_list(
            playlists
                .iter()
                .map(|p| GetPlaylist(p.name.clone()))
                .collect::<Vec<_>>(),
        )
        .await?;

    Ok(GetPlaylists {
        playlists: playlists
            .iter()
            .zip(playlists_info)
            .map(|(p, info)| Playlist {
                id: PlaylistID::new(&p.name),
                name: p.name.clone(),
                owner: params.u.clone(),
                public: true,
                song_count: info.len() as u32,
                duration: info
                    .iter()
                    .map(|s| s.duration.map(|v| v.as_secs()).unwrap_or(0))
                    .sum(),
                changed: p.last_modified.to_rfc3339(),
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
    song_count: u32,
    #[yaserde(attribute)]
    duration: u64,
    #[yaserde(attribute)]
    changed: String,
}

#[cfg(test)]
mod tests {
    use super::{GetPlaylists, Playlist};
    use crate::api::{expect_ok_json, expect_ok_xml, json, types::PlaylistID, xml};
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
}
