use axum::{
    body::Body,
    routing::{get, post},
    http::Request,
    extract::{Extension, Path},
    Router, response::IntoResponse,
};
use hyper::StatusCode;

use crate::{channel::router::{ChannelRouter, CommandResponse}, message::Message};

async fn create_channel(
    Extension(channel_router): Extension<ChannelRouter>,
    req: Request<Body>
) -> impl IntoResponse {
    // Your implementation here
}

async fn get_channels(Extension(channel_router): Extension<ChannelRouter>) -> impl IntoResponse {
    let res = channel_router.send_command(Message::ChannelGetAll, None).await;

    match res {
        Ok(CommandResponse::ChannelGetAll(channels)) => {
            let body = serde_json::to_string(&channels).unwrap();
            (StatusCode::OK, body).into_response()
        }
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_channel(
    Path(id): Path<u64>,
    Extension(channel_router): Extension<ChannelRouter>,
) -> impl IntoResponse {
    // Your implementation here
}

async fn update_channel(
    Path(id): Path<u64>,
    Extension(channel_router): Extension<ChannelRouter>,
    req: Request<Body>,
) -> impl IntoResponse {
    // Your implementation here
}

async fn delete_channel(
    Path(id): Path<u64>,
    Extension(channel_router): Extension<ChannelRouter>,
) -> impl IntoResponse {
    // Your implementation here
}

pub fn channel_routes(channels: ChannelRouter) -> Router {
    Router::new()
        .route("/channels", post(create_channel).get(get_channels))
        .route("/channels/:id", get(get_channel).put(update_channel).delete(delete_channel))
        .layer(Extension(channels))
}
