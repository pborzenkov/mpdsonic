use axum::{extract::Query, routing::Router};
use serde::{Deserialize, Serialize};
use yaserde_derive::YaSerialize;

pub fn get_router() -> Router {
    Router::new().route("/getUser.view", super::handler(get_user))
}

#[derive(Clone, Deserialize)]
struct GetUserQuery {
    u: String,
    username: String,
}

async fn get_user(Query(params): Query<GetUserQuery>) -> super::Result<GetUser> {
    match params.u == params.username {
        true => Ok(GetUser {
            username: params.username.clone(),
            scrobbling_enabled: false,
            admin_role: false,
            settings_role: false,
            download_role: true,
            upload_role: false,
            playlist_role: true,
            cover_art_role: true,
            comment_role: false,
            podcast_role: false,
            stream_role: true,
            jukebox_role: false,
            share_role: false,
            video_conversion_role: false,
            folder: vec!["/".to_string()],
        }),
        false => Err(super::Error::not_authorized(&format!(
            "{} is not authorized to get details for other users.",
            params.u
        ))),
    }
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "user")]
#[serde(rename_all = "camelCase")]
struct GetUser {
    #[yaserde(attribute)]
    username: String,
    #[yaserde(attribute, rename = "scrobblingEnabled")]
    scrobbling_enabled: bool,
    #[yaserde(attribute, rename = "adminRole")]
    admin_role: bool,
    #[yaserde(attribute, rename = "settingsRole")]
    settings_role: bool,
    #[yaserde(attribute, rename = "downloadRole")]
    download_role: bool,
    #[yaserde(attribute, rename = "uploadRole")]
    upload_role: bool,
    #[yaserde(attribute, rename = "playlistRole")]
    playlist_role: bool,
    #[yaserde(attribute, rename = "coverArtRole")]
    cover_art_role: bool,
    #[yaserde(attribute, rename = "commentRole")]
    comment_role: bool,
    #[yaserde(attribute, rename = "podcastRole")]
    podcast_role: bool,
    #[yaserde(attribute, rename = "streamRole")]
    stream_role: bool,
    #[yaserde(attribute, rename = "jukeboxRole")]
    jukebox_role: bool,
    #[yaserde(attribute, rename = "shareRole")]
    share_role: bool,
    #[yaserde(attribute, rename = "videoConversionRole")]
    video_conversion_role: bool,
    #[yaserde(child)]
    folder: Vec<String>,
}

impl super::Reply for GetUser {
    fn field_name() -> Option<&'static str> {
        Some("user")
    }
}

#[cfg(test)]
mod tests {
    use super::GetUser;
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};
    use serde_json::json;

    #[test]
    fn get_user() {
        let get_user = GetUser {
            username: "test".to_string(),
            scrobbling_enabled: false,
            admin_role: false,
            settings_role: false,
            download_role: true,
            upload_role: false,
            playlist_role: true,
            cover_art_role: true,
            comment_role: false,
            podcast_role: false,
            stream_role: true,
            jukebox_role: false,
            share_role: false,
            video_conversion_role: false,
            folder: vec!["/".to_string()],
        };
        assert_eq!(
            xml(&get_user),
            expect_ok_xml(Some(
                r#"<user username="test" scrobblingEnabled="false" adminRole="false" settingsRole="false" downloadRole="true" uploadRole="false" playlistRole="true" coverArtRole="true" commentRole="false" podcastRole="false" streamRole="true" jukeboxRole="false" shareRole="false" videoConversionRole="false">
    <folder>/</folder>
  </user>"#
            ),)
        );

        assert_eq!(
            json(&get_user),
            expect_ok_json(Some(json!({"user": {
            "username": "test",
            "scrobblingEnabled": false,
            "adminRole": false,
            "settingsRole": false,
            "downloadRole": true,
            "uploadRole": false,
            "playlistRole": true,
            "coverArtRole": true,
            "commentRole": false,
            "podcastRole": false,
            "streamRole": true,
            "jukeboxRole": false,
            "shareRole": false,
            "videoConversionRole": false,
            "folder": ["/"],
            }})),),
        );
    }
}
