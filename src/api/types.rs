use base64::DecodeError;
use serde::{Deserialize, Serialize};
use std::fmt;

pub enum IDError {
    SerializationFailed(serde_json::Error),
    DecodingFailed(DecodeError),
    DeserializationFailed(serde_json::Error),
}

impl fmt::Display for IDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IDError::SerializationFailed(e) => write!(f, "Failed to serialize: {}", e),
            IDError::DecodingFailed(e) => write!(f, "Failed to decode: {}", e),
            IDError::DeserializationFailed(e) => write!(f, "Failed to deserialize: {}", e),
        }
    }
}

macro_rules! api_id {
    ($id:ty) => {
        impl TryInto<String> for $id {
            type Error = IDError;

            fn try_into(self) -> Result<String, Self::Error> {
                use serde_json::ser::Serializer;

                let mut ser = Serializer::new(Vec::with_capacity(32));

                // This should never fail
                <$id>::serialize(&self, &mut ser).map_err(IDError::SerializationFailed)?;
                Ok(base64::encode(ser.into_inner()))
            }
        }

        impl TryFrom<String> for $id {
            type Error = IDError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                use serde_json::de::Deserializer;

                let decoded = base64::decode(&s).map_err(IDError::DecodingFailed)?;
                let mut de = Deserializer::from_slice(&decoded);
                <$id>::deserialize(&mut de).map_err(IDError::DeserializationFailed)
            }
        }

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

        impl<'de> Deserialize<'de> for $id {
            fn deserialize<D>(deserializer: D) -> Result<$id, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::Error;

                if deserializer.is_human_readable() {
                    let s: String = serde::Deserialize::deserialize(deserializer)?;
                    s.try_into().map_err(D::Error::custom)
                } else {
                    panic!("did't expect non-human readable form");
                }
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

// ArtistID identifies an artist
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub struct ArtistID {
    pub name: String,
}

impl ArtistID {
    pub fn new(name: &str) -> Self {
        ArtistID {
            name: name.to_string(),
        }
    }
}

// AlbumID identifies an album
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub struct AlbumID {
    pub name: String,
    pub artist: String,
}

impl AlbumID {
    pub fn new(name: &str, artist: &str) -> Self {
        AlbumID {
            name: name.to_string(),
            artist: artist.to_string(),
        }
    }
}

// ArtworkID identifies an artwork
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(remote = "Self")]
pub struct ArtworkID {
    pub path: String,
}

impl ArtworkID {
    pub fn new(path: &str) -> Self {
        ArtworkID {
            path: path.to_string(),
        }
    }
}

api_id!(ArtistID);
api_id!(AlbumID);
api_id!(ArtworkID);
