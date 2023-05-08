#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct AuthPayload {
    pub channel: String,
    pub conn_id: String,
}

pub async fn make_auth_request(config: &AuthConfig, payload: AuthPayload) -> Result<bool, String> {
    println!("Authenticating with url: {}", config.url);
    println!("Authenticating with payload: {:?}", payload);
    Ok(false)
}
