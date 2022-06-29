use axum::routing::Router;
use serde::Serialize;
use yaserde_derive::YaSerialize;

pub fn get_router() -> Router {
    Router::new().route("/getMusicFolders.view", super::handler(get_music_folders))
}

async fn get_music_folders() -> super::Result<GetMusicFolders> {
    Ok(GetMusicFolders {
        music_folders: vec![MusicFolder {
            id: "/".to_string(),
            name: "Music".to_string(),
        }],
    })
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "musicFolder")]
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

#[cfg(test)]
mod tests {
    use super::{GetMusicFolders, MusicFolder};
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};
    use serde_json::json;

    #[test]
    fn get_user() {
        let get_user = GetMusicFolders {
            music_folders: vec![MusicFolder {
                id: "/".to_string(),
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
                    "id": "/",
                    "name": "Music",
                    }
                ]
                }
            })),),
        );
    }
}
