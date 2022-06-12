use axum::extract::rejection;
use serde::Serialize;
use yaserde_derive::YaSerialize;

// An API error response
#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "error")]
pub struct Error {
    #[yaserde(attribute)]
    code: u32,
    #[yaserde(attribute)]
    message: String,
}

impl Error {
    pub fn new(code: u32, message: &str) -> Self {
        Error {
            code,
            message: message.to_string(),
        }
    }

    pub fn missing_parameter() -> Self {
        Error::new(10, "Required parameter is missing")
    }

    pub fn authentication_failed() -> Self {
        Error::new(40, "Wrong username or password")
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

impl From<rejection::QueryRejection> for Error {
    fn from(_: rejection::QueryRejection) -> Self {
        Error::missing_parameter()
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
