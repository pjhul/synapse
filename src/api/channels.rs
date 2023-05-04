use axum::{
    body::Body,
    routing::{get, post, put, delete},
    http::{Request, Response},
    extract::{Extension, Path},
    Router, response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::channel::ChannelRouter;

async fn create_channel(
    Extension(channel_router): Extension<ChannelRouter>,
    req: Request<Body>
) -> impl IntoResponse {
    // Your implementation here
}

async fn get_channels(Extension(channel_router): Extension<ChannelRouter>) -> impl IntoResponse {
    println!("get_channels");
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
