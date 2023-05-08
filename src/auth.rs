use hyper::StatusCode;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    pub url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Join,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthPayload {
    pub operation: Operation,
    pub channel: String,
    pub conn_id: String,
}

pub async fn make_auth_request(config: &AuthConfig, payload: AuthPayload) -> Result<bool, String> {
    let res = reqwest::Client::new()
        .post(&config.url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    info!("Received auth status {}", res.status());

    let is_success = res.status() == StatusCode::OK; // 200

    Ok(is_success)
}
