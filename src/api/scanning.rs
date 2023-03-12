use crate::mpd::commands::Update;
use axum::{routing::Router, Extension};
use mpd_client::commands::{Stats, Status};
use serde::Serialize;
use std::sync::Arc;
use yaserde_derive::YaSerialize;

pub(crate) fn get_router() -> Router {
    Router::new()
        .route("/startScan.view", super::handler(start_scan))
        .route("/getScanStatus.view", super::handler(get_scan_status))
}

async fn start_scan(Extension(state): Extension<Arc<super::State>>) -> super::Result<ScanStatus> {
    let (_, stats, status) = state
        .pool
        .get()
        .await?
        .command_list((Update::new(), Stats, Status))
        .await?;

    Ok(ScanStatus {
        scanning: status.update_job.is_some(),
        count: stats.songs,
    })
}

#[derive(Serialize, YaSerialize)]
#[yaserde(rename = "scanStatus")]
struct ScanStatus {
    #[yaserde(attribute)]
    scanning: bool,
    #[yaserde(attribute)]
    count: u64,
}

impl super::Reply for ScanStatus {
    fn field_name() -> Option<&'static str> {
        Some("scanStatus")
    }
}

async fn get_scan_status(
    Extension(state): Extension<Arc<super::State>>,
) -> super::Result<ScanStatus> {
    let (stats, status) = state
        .pool
        .get()
        .await?
        .command_list((Stats, Status))
        .await?;

    Ok(ScanStatus {
        scanning: status.update_job.is_some(),
        count: stats.songs,
    })
}

#[cfg(test)]
mod tests {
    use super::ScanStatus;
    use crate::api::{expect_ok_json, expect_ok_xml, json, xml};
    use serde_json::json;

    #[test]
    fn start_scan() {
        let scan_status = ScanStatus {
            scanning: true,
            count: 1234,
        };
        assert_eq!(
            xml(&scan_status),
            expect_ok_xml(Some(r#"<scanStatus scanning="true" count="1234" />"#),),
        );

        assert_eq!(
            json(&scan_status),
            expect_ok_json(Some(json!({"scanStatus": {
                "scanning": true,
                "count": 1234,
            }
            })),),
        );
    }
}
