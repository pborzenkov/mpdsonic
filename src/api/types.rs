use base64::{DecodeError, Engine};
use serde::{Deserialize, Serialize};
use std::fmt;
use yaserde_derive::YaSerialize;

pub(crate) enum IDError {
    Serialization(serde_json::Error),
    Decoding(DecodeError),
    Deserialization(serde_json::Error),
}

static BASE64: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    base64::engine::GeneralPurposeConfig::new(),
);

impl fmt::Display for IDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IDError::Serialization(e) => write!(f, "Failed to serialize: {e}"),
            IDError::Decoding(e) => write!(f, "Failed to decode: {e}"),
            IDError::Deserialization(e) => write!(f, "Failed to deserialize: {e}"),
        }
    }
}

macro_rules! api_id_into_string {
    ($id:ty) => {
        impl TryInto<String> for $id {
            type Error = IDError;

            fn try_into(self) -> Result<String, Self::Error> {
                use serde_json::ser::Serializer;

                let mut ser = Serializer::new(Vec::with_capacity(32));

                // This should never fail
                <$id>::serialize(&self, &mut ser).map_err(IDError::Serialization)?;
                Ok(BASE64.encode(ser.into_inner()))
            }
        }
    };
}

macro_rules! api_id_from_string {
    ($id:ty) => {
        impl TryFrom<&str> for $id {
            type Error = IDError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                use serde_json::de::Deserializer;

                let decoded = BASE64.decode(s).map_err(IDError::Decoding)?;
                let mut de = Deserializer::from_slice(&decoded);
                <$id>::deserialize(&mut de).map_err(IDError::Deserialization)
            }
        }
    };
}

macro_rules! api_id_serialize {
    ($id:ty) => {
        impl Serialize for $id {
            fn serialize<'a, S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                use serde::ser::Error;

                let val: String = self.clone().try_into().map_err(S::Error::custom)?;
                serializer.serialize_str(&val)
            }
        }

        impl yaserde::YaSerialize for $id {
            fn serialize<W: std::io::Write>(
                &self,
                writer: &mut yaserde::ser::Serializer<W>,
            ) -> std::result::Result<(), String> {
                use xml::writer::XmlEvent;

                let val: String = self
                    .clone()
                    .try_into()
                    .map_err(|e: IDError| e.to_string())?;
                writer
                    .write(XmlEvent::characters(&val))
                    .map_err(|e| e.to_string())
            }

            fn serialize_attributes(
                &self,
                attributes: Vec<xml::attribute::OwnedAttribute>,
                namespace: xml::namespace::Namespace,
            ) -> std::result::Result<
                (
                    Vec<xml::attribute::OwnedAttribute>,
                    xml::namespace::Namespace,
                ),
                String,
            > {
                Ok((attributes, namespace))
            }
        }
    };
}

macro_rules! api_id_deserialize {
    ($id:ty) => {
        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<$id, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::Error;

                if deserializer.is_human_readable() {
                    let s: String = serde::Deserialize::deserialize(deserializer)?;
                    let tmp = s.as_str().try_into().map_err(D::Error::custom);
                    tmp
                } else {
                    panic!("did't expect non-human readable form");
                }
            }
        }
    };
}

macro_rules! api_id {
    ($id:ty) => {
        api_id_into_string!($id);
        api_id_from_string!($id);
        api_id_serialize!($id);
        api_id_deserialize!($id);
    };
}

// ArtistID identifies an artist
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub(crate) struct ArtistID {
    pub(crate) name: String,
}

impl ArtistID {
    pub(crate) fn new(name: &str) -> Self {
        ArtistID {
            name: name.to_string(),
        }
    }
}

// AlbumID identifies an album
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub(crate) struct AlbumID {
    pub(crate) name: String,
    pub(crate) artist: String,
}

impl AlbumID {
    pub(crate) fn new(name: &str, artist: &str) -> Self {
        AlbumID {
            name: name.to_string(),
            artist: artist.to_string(),
        }
    }
}

// SongID identifies a song
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub(crate) struct SongID {
    pub(crate) path: String,
}

impl SongID {
    pub(crate) fn new(path: &str) -> Self {
        SongID {
            path: path.to_string(),
        }
    }
}

// ArtworkID identifies an artwork
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(remote = "Self")]
#[serde(untagged)]
pub(crate) enum CoverArtID {
    Song { path: String },
    Playlist { name: String },
}

impl CoverArtID {
    pub(crate) fn new(path: &str) -> Self {
        CoverArtID::Song {
            path: path.to_string(),
        }
    }
}

impl Default for CoverArtID {
    fn default() -> Self {
        CoverArtID::new("")
    }
}

impl TryFrom<&str> for CoverArtID {
    type Error = IDError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        use serde_json::de::Deserializer;

        // Handle the way DSub requests playlist cover art (pl-<playlistid>)
        let s = s.trim_start_matches("pl-");
        let decoded = BASE64.decode(s).map_err(IDError::Decoding)?;
        let mut de = Deserializer::from_slice(&decoded);
        CoverArtID::deserialize(&mut de).map_err(IDError::Deserialization)
    }
}

// PlaylistID identifies a playlist
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub(crate) struct PlaylistID {
    pub(crate) name: String,
}

impl PlaylistID {
    pub(crate) fn new(name: &str) -> Self {
        PlaylistID {
            name: name.to_string(),
        }
    }
}

api_id!(ArtistID);
api_id!(AlbumID);
api_id!(SongID);
api_id_into_string!(CoverArtID);
api_id_serialize!(CoverArtID);
api_id_deserialize!(CoverArtID);
api_id!(PlaylistID);

#[derive(Serialize, YaSerialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Song {
    #[yaserde(attribute)]
    pub(crate) id: SongID,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) album: Option<String>,
    #[yaserde(attribute)]
    pub(crate) artist: String,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) track: Option<u32>,
    #[yaserde(attribute, rename = "discNumber")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) disc_number: Option<u32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) year: Option<i32>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) genre: Option<String>,
    #[yaserde(attribute, rename = "coverArt")]
    pub(crate) cover_art: CoverArtID,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duration: Option<u64>,
    #[yaserde(attribute)]
    pub(crate) path: String,
    #[yaserde(attribute, rename = "albumId")]
    pub(crate) album_id: Option<AlbumID>,
    #[yaserde(attribute, rename = "artistId")]
    pub(crate) artist_id: ArtistID,
    #[yaserde(attribute, rename = "userRating")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_rating: Option<u8>,
    #[yaserde(attribute)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) starred: Option<String>,
}
