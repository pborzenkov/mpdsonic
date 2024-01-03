use super::{library::Library, mpd::ConnectionManager};
use crate::listenbrainz;
use axum::{
    body::Body,
    extract::{Extension, FromRequestParts, Query},
    http::{header, request::Parts, HeaderValue, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{on_service, MethodFilter, MethodRouter, Router},
};
use bb8::Pool;
use glue::{Handler, RawHandler};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

mod annotation;
mod browsing;
mod common;
mod error;
mod glue;
mod playlists;
mod retrieval;
mod scanning;
mod system;
mod types;
mod users;

static VERSION: &str = "1.16.1";

use error::Error;

// Result returned by an API handler
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub(crate) struct Authentication {
    username: String,
    password: String,
    encoded_password: String,
}

struct State {
    pool: Pool<ConnectionManager>,
    lib: Box<dyn Library + Send + Sync>,
    listenbrainz: Option<listenbrainz::Client>,
}

impl Authentication {
    pub(crate) fn new(username: &str, password: &str) -> Self {
        Authentication {
            username: username.to_string(),
            password: password.to_string(),
            encoded_password: format!("enc:{}", hex::encode(password)),
        }
    }
}

pub(crate) fn get_router(
    auth: Authentication,
    pool: Pool<ConnectionManager>,
    lib: Box<dyn Library + Send + Sync>,
    listenbrainz: Option<listenbrainz::Client>,
) -> Router {
    Router::new()
        .nest(
            "/rest",
            Router::new()
                .merge(annotation::get_router())
                .merge(browsing::get_router())
                .merge(playlists::get_router())
                .merge(retrieval::get_router())
                .merge(scanning::get_router())
                .merge(system::get_router())
                .merge(users::get_router()),
        )
        .route_layer(middleware::from_fn(move |req, next| {
            authenticate(req, next, auth.clone())
        }))
        .layer(CorsLayer::new().allow_origin(Any))
        .layer(Extension(Arc::new(State {
            pool,
            lib,
            listenbrainz,
        })))
}

// handler converts an API handler into a MethodRouter which can be provided to axum's router
fn handler<H, T>(handler: H) -> MethodRouter
where
    H: Handler<T, ()>,
    T: Clone + 'static,
{
    on_service(
        MethodFilter::GET.or(MethodFilter::POST),
        handler.into_service(),
    )
}

// raw_handler converts a raw API handler into a MethodRouter which can be provided to axum's router
fn raw_handler<H, T>(handler: H) -> MethodRouter
where
    H: RawHandler<T, ()>,
    T: Clone + 'static,
{
    on_service(
        MethodFilter::GET.or(MethodFilter::POST),
        handler.into_service(),
    )
}

#[derive(Deserialize)]
struct AuthenticationQuery {
    u: String,
    p: Option<String>,
    t: Option<String>,
    s: Option<String>,
}

async fn authenticate(req: Request<Body>, next: Next, auth: Authentication) -> Response {
    use constant_time_eq::constant_time_eq;

    let (mut parts, body) = req.into_parts();

    let aq = Query::<AuthenticationQuery>::from_request_parts(&mut parts, &()).await;
    let err: Option<Error> = if let Ok(aq) = aq {
        let valid_user = constant_time_eq(aq.u.as_bytes(), auth.username.as_bytes());

        match (aq.p.as_deref(), aq.t.as_deref(), aq.s.as_deref()) {
            (Some(p), _, _)
                if p.starts_with("enc:")
                    && constant_time_eq(p.as_bytes(), auth.encoded_password.as_bytes())
                    && valid_user =>
            {
                None
            }
            (Some(p), _, _) if p.starts_with("enc:") => Some(Error::authentication_failed()),
            (Some(p), _, _)
                if constant_time_eq(p.as_bytes(), auth.password.as_bytes()) && valid_user =>
            {
                None
            }
            (Some(_), _, _) => Some(Error::authentication_failed()),
            (_, Some(t), Some(s))
                if constant_time_eq(
                    t.as_bytes(),
                    format!("{:?}", md5::compute(auth.password + s)).as_bytes(),
                ) && valid_user =>
            {
                None
            }
            (_, Some(_), Some(_)) => Some(Error::authentication_failed()),
            _ => Some(Error::missing_parameter(
                "either username or password is missing",
            )),
        }
    } else {
        aq.err().map(Into::into)
    };
    if let Some(err) = err {
        return serialize_reply(err, &serialization_format(&parts));
    }

    next.run(Request::from_parts(parts, body)).await
}

// Trait for data that can be returned as API reply
trait Reply: yaserde::YaSerialize + serde::Serialize {
    fn is_error() -> bool {
        false
    }
    fn field_name() -> Option<&'static str>;
}

// Optional query values controlling response serialization format.
#[derive(Default, Deserialize)]
struct SerializationQuery {
    f: Option<String>,
    callback: Option<String>,
}

fn serialization_format(req: &Parts) -> SerializationQuery {
    let query = req.uri.query().unwrap_or_default();

    // Official Subsonic server falls back to XML if some of the parameters are invalid or not provided
    serde_urlencoded::from_str::<SerializationQuery>(query).unwrap_or_default()
}

fn serialize_reply<T>(reply: T, format: &SerializationQuery) -> Response
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
