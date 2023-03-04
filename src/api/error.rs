use crate::{library, listenbrainz, mpd};
use axum::extract::rejection;
use serde::Serialize;
use std::convert::Infallible;
use yaserde_derive::YaSerialize;

// An API error response
#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "error")]
pub(crate) struct Error {
    #[yaserde(attribute)]
    code: u32,
    #[yaserde(attribute)]
    message: String,
}

impl Error {
    pub(crate) fn new(code: u32, message: &str) -> Self {
        Error {
            code,
            message: message.to_string(),
        }
    }

    pub(crate) fn generic_error(msg: Option<&str>) -> Self {
        match msg {
            Some(msg) => Error::new(0, &format!("A generic error: {msg}")),
            None => Error::new(0, "A generic error"),
        }
    }

    pub(crate) fn missing_parameter(msg: &str) -> Self {
        Error::new(10, &format!("Required parameter is missing: {msg}"))
    }

    pub(crate) fn authentication_failed() -> Self {
        Error::new(40, "Wrong username or password")
    }

    pub(crate) fn not_authorized(msg: &str) -> Self {
        Error::new(50, msg)
    }

    pub(crate) fn not_found() -> Self {
        Error::new(70, "The requested data was not found")
    }
}

impl super::Reply for Error {
    fn is_error() -> bool {
        true
    }
    fn field_name() -> Option<&'static str> {
        Some("error")
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        panic!("Must never happen")
    }
}

impl From<rejection::QueryRejection> for Error {
    fn from(err: rejection::QueryRejection) -> Self {
        Error::missing_parameter(&err.to_string())
    }
}

impl From<rejection::ExtensionRejection> for Error {
    fn from(_: rejection::ExtensionRejection) -> Self {
        Error::generic_error(None)
    }
}

impl From<mpd_client::client::CommandError> for Error {
    fn from(err: mpd_client::client::CommandError) -> Self {
        // TODO: handle specific cases
        Error::generic_error(Some(&err.to_string()))
    }
}

impl From<bb8::RunError<mpd::Error>> for Error {
    fn from(err: bb8::RunError<mpd::Error>) -> Self {
        Error::generic_error(Some(&err.to_string()))
    }
}

impl From<library::Error> for Error {
    fn from(err: library::Error) -> Self {
        if err.is_not_found() {
            Error::not_found()
        } else {
            Error::generic_error(None)
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::generic_error(Some(&err.to_string()))
    }
}

impl From<listenbrainz::Error> for Error {
    fn from(err: listenbrainz::Error) -> Self {
        Error::generic_error(Some(&err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use crate::api::{expect_json, expect_xml, json, xml};
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
