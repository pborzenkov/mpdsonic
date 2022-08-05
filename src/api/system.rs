use axum::routing::Router;
use serde::Serialize;
use yaserde_derive::YaSerialize;

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/ping.view", super::handler(ping))
        .route("/getLicense.view", super::handler(get_license))
}

async fn ping() -> super::Result<()> {
    Ok(())
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "license")]
struct License {
    #[yaserde(attribute)]
    valid: bool,
}

impl super::Reply for License {
    fn field_name() -> Option<&'static str> {
        Some("license")
    }
}

async fn get_license() -> super::Result<License> {
    Ok(License { valid: true })
}

#[cfg(test)]
mod tests {
    use super::License;
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};
    use serde_json::json;

    #[test]
    fn license() {
        let lic = License { valid: true };
        assert_eq!(
            xml(&lic),
            expect_ok_xml(Some(r#"<license valid="true" />"#),)
        );

        assert_eq!(
            json(&lic),
            expect_ok_json(Some(json!({"license": {
            "valid": true,
            }})),),
        );
    }
}
