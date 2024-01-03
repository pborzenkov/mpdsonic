use mpd_client::{responses::Song, tag::Tag};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::Serialize;
use std::{collections::HashMap, fmt};

#[derive(Clone)]
pub(crate) struct Client {
    client: reqwest::Client,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub(crate) enum Error {
    Http(reqwest::Error),
    Header(header::InvalidHeaderValue),
    Song,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Http(err)
    }
}

impl From<header::InvalidHeaderValue> for Error {
    fn from(err: header::InvalidHeaderValue) -> Self {
        Error::Header(err)
    }
}

impl Client {
    pub(crate) fn new(token: &str) -> Result<Client> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Token {token}"))?,
        );

        Ok(Client {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()?,
        })
    }

    pub(crate) async fn listen(&self, song: &Song, timestamp: i64) -> Result<()> {
        self.submit(Submission::Listen([Listen {
            listened_at: timestamp,
            track_metadata: metadata_from_song(song).ok_or(Error::Song)?,
        }]))
        .await
    }

    pub(crate) async fn playing_now(&self, song: &Song) -> Result<()> {
        self.submit(Submission::PlayingNow([PlayingNow {
            track_metadata: metadata_from_song(song).ok_or(Error::Song)?,
        }]))
        .await
    }

    pub(crate) async fn feedback(&self, song: &Song, score: Score) -> Result<()> {
        let feedback = Feedback {
            recording_mbid: single_value(&song.tags, Tag::MusicBrainzRecordingId),
            score,
        };

        self.client
            .post("https://api.listenbrainz.org/1/feedback/recording-feedback")
            .json(&feedback)
            .send()
            .await?;

        Ok(())
    }

    async fn submit(&self, submission: Submission) -> Result<()> {
        self.client
            .post("https://api.listenbrainz.org/1/submit-listens")
            .json(&submission)
            .send()
            .await?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "listen_type", content = "payload")]
enum Submission {
    #[serde(rename = "single")]
    Listen([Listen; 1]),
    #[serde(rename = "playing_now")]
    PlayingNow([PlayingNow; 1]),
}

fn metadata_from_song(song: &Song) -> Option<TrackMetadata> {
    Some(TrackMetadata {
        artist_name: single_value(&song.tags, Tag::Artist)?,
        track_name: single_value(&song.tags, Tag::Title)?,
        release_name: single_value(&song.tags, Tag::Album),
        additional_info: AdditionalInfo {
            artist_mbids: song
                .tags
                .get(&Tag::MusicBrainzArtistId)
                .cloned()
                .unwrap_or_default(),
            release_mbid: single_value(&song.tags, Tag::MusicBrainzReleaseId),
            recording_mbid: single_value(&song.tags, Tag::MusicBrainzRecordingId),
            track_mbid: single_value(&song.tags, Tag::MusicBrainzTrackId),
            work_mbids: song
                .tags
                .get(&Tag::MusicBrainzWorkId)
                .cloned()
                .unwrap_or_default(),
            tracknumber: single_value(&song.tags, Tag::Track),
            duration_ms: song.duration.map(|d| d.as_millis()),
            media_player: "mpdsonic",
            submission_client: env!("CARGO_PKG_NAME"),
            submission_client_version: env!("CARGO_PKG_VERSION"),
        },
    })
}

fn single_value(tags: &HashMap<Tag, Vec<String>>, tag: Tag) -> Option<String> {
    match tags.get(&tag) {
        Some(v) if !v.is_empty() => v.first().cloned(),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
pub struct Listen {
    listened_at: i64,
    track_metadata: TrackMetadata,
}

#[derive(Debug, Serialize)]
pub struct PlayingNow {
    track_metadata: TrackMetadata,
}

#[derive(Debug, Serialize)]
struct TrackMetadata {
    artist_name: String,
    track_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_name: Option<String>,
    additional_info: AdditionalInfo,
}

#[derive(Debug, Serialize)]
struct AdditionalInfo {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    artist_mbids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_mbid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recording_mbid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    track_mbid: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    work_mbids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tracknumber: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u128>,
    media_player: &'static str,
    submission_client: &'static str,
    submission_client_version: &'static str,
}

#[derive(Debug, Serialize)]
struct Feedback {
    #[serde(skip_serializing_if = "Option::is_none")]
    recording_mbid: Option<String>,
    #[serde(serialize_with = "serialize_score")]
    score: Score,
}

#[derive(Debug, Serialize)]
pub(crate) enum Score {
    Love,
    Hate,
    Remove,
}

fn serialize_score<S>(score: &Score, s: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_i32(match score {
        Score::Love => 1,
        Score::Hate => -1,
        Score::Remove => 0,
    })
}
