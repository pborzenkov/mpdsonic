use axum::{
    body::Body,
    response::{IntoResponse, Response},
    routing::{on_service, MethodFilter, MethodRouter, Router},
};
use glue::Handler;
use http::header::{self, HeaderValue};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tower_http::cors::{Any, CorsLayer};
use yaserde_derive::YaSerialize;

mod glue;
mod system;

static VERSION: &str = "1.16.1";

// Result returned by an API handler
type Result<T> = std::result::Result<T, Error>;

pub fn get_router() -> Router {
    Router::new()
        .nest("/rest", Router::new().merge(system::get_router()))
        .layer(CorsLayer::new().allow_origin(Any))
}

// handler converts an API handler into a MethodRouter which can be provided to axum's router
fn handler<H, T>(handler: H) -> MethodRouter<Body, Infallible>
where
    H: Handler<T>,
    T: Clone + 'static,
{
    on_service(
        MethodFilter::GET | MethodFilter::POST,
        handler.into_service(),
    )
}

// Trait for data that can be returned as API reply
trait Reply: yaserde::YaSerialize + serde::Serialize {
    fn is_error() -> bool {
        false
    }
    fn field_name() -> Option<&'static str>;
}

// An API error response
#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "error")]
struct Error {
    #[yaserde(attribute)]
    code: u32,
    #[yaserde(attribute)]
    message: String,
}

impl Reply for Error {
    fn is_error() -> bool {
        true
    }
    fn field_name() -> Option<&'static str> {
        Some("error")
    }
}

// Optional query values controlling response serialization format.
#[derive(Default, Deserialize)]
struct SerializationFormat {
    f: Option<String>,
    callback: Option<String>,
}

fn serialize_reply<T>(reply: T, format: &SerializationFormat) -> Response
where
    T: Reply,
{
    match (format.f.as_deref(), &format.callback) {
        (Some("json"), _) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::APPLICATION_JSON.as_ref()),
            )],
            json(&reply),
        )
            .into_response(),
        (Some("jsonp"), Some(callback)) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::TEXT_JAVASCRIPT.as_ref()),
            )],
            format!(
                "{callback}({data})",
                callback = callback,
                data = json(&reply)
            ),
        )
            .into_response(),
        _ => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(mime::TEXT_XML.as_ref()),
            )],
            xml(&reply),
        )
            .into_response(),
    }
}

fn xml<T>(reply: &T) -> String
where
    T: Reply,
{
    use yaserde::ser::{to_string_with_config, Config, Serializer};

    struct Response<'a, T>(&'a T);

    impl<'a, T> yaserde::YaSerialize for Response<'a, T>
    where
        T: Reply,
    {
        fn serialize<W: std::io::Write>(
            &self,
            writer: &mut Serializer<W>,
        ) -> std::result::Result<(), String> {
            use xml::writer::XmlEvent;

            writer
                .write(
                    XmlEvent::start_element("subsonic-response")
                        .attr("xmlns", "http://subsonic.org/restapi")
                        .attr(
                            "status",
                            if <T as Reply>::is_error() {
                                "failed"
                            } else {
                                "ok"
                            },
                        )
                        .attr("version", VERSION),
                )
                .map_err(|err| err.to_string())?;
            if <T as Reply>::field_name().is_some() {
                yaserde::YaSerialize::serialize(self.0, writer)?;
            }
            writer
                .write(XmlEvent::end_element())
                .map_err(|err| err.to_string())?;
            Ok(())
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

    to_string_with_config(
        &Response(reply),
        &Config {
            perform_indent: true,
            ..Default::default()
        },
    )
    .expect("failed to serialize XML reply")
}

fn json<T>(reply: &T) -> String
where
    T: Reply,
{
    use serde::ser::{SerializeMap, Serializer};
    use serde_json::to_string_pretty;

    struct InnerResponse<'a, T>(&'a T);

    #[derive(Serialize)]
    struct Response<'a, T: Reply> {
        #[serde(rename(serialize = "subsonic-response"))]
        sr: InnerResponse<'a, T>,
    }

    impl<'a, T> Serialize for InnerResponse<'a, T>
    where
        T: Reply,
    {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(3))?;
            map.serialize_entry(
                "status",
                if <T as Reply>::is_error() {
                    "failed"
                } else {
                    "ok"
                },
            )?;
            map.serialize_entry("version", VERSION)?;
            if let Some(field) = <T as Reply>::field_name() {
                map.serialize_entry(field, &self.0)?;
            }
            map.end()
        }
    }

    to_string_pretty(&Response {
        sr: InnerResponse(reply),
    })
    .expect("failed to serialize JSON reply")
}

#[cfg(test)]
fn expect_xml(inner: Option<&str>, status: &str) -> String {
    match inner {
        Some(inner) => format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="{status}" version="{version}">
  {inner}
</subsonic-response>"#,
            status = status,
            version = VERSION,
            inner = inner
        ),
        None => format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<subsonic-response xmlns="http://subsonic.org/restapi" status="{status}" version="{version}" />"#,
            status = status,
            version = VERSION,
        ),
    }
}
#[cfg(test)]
fn expect_ok_xml(inner: Option<&str>) -> String {
    expect_xml(inner, "ok")
}

#[cfg(test)]
fn expect_json(inner: Option<serde_json::Value>, status: &str) -> String {
    use serde_json::{json, to_string_pretty};

    let mut exp = json!({
    "subsonic-response": {
    "status": status,
    "version": VERSION,
    }});

    if let Some(inner) = inner {
        for (k, v) in inner.as_object().unwrap() {
            exp["subsonic-response"]
                .as_object_mut()
                .unwrap()
                .insert(k.clone(), v.clone());
        }
    }

    to_string_pretty(&exp).unwrap()
}

#[cfg(test)]
fn expect_ok_json(inner: Option<serde_json::Value>) -> String {
    expect_json(inner, "ok")
}

#[cfg(test)]
mod tests {
    use super::{expect_json, expect_xml, json, xml, Error};
    use serde_json::json;

    #[test]
    fn empty() {
        let err = Error {
            code: 10,
            message: "missing parameter".to_string(),
        };
        assert_eq!(
            xml(&err),
            expect_xml(
                Some(r#"<error code="10" message="missing parameter" />"#),
                "failed"
            )
        );

        assert_eq!(
            json(&err),
            expect_json(
                Some(json!({"error": {
                "code": 10,
                "message": "missing parameter",
                }})),
                "failed",
            ),
        );
    }
}
