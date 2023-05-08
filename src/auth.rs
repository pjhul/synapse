#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub url: String,
}

pub struct AuthPayload {
    channel: String,
    user_id: String,
}
