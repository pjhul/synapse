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
    // FIXME: This should be a JSON body with a name field and not raw text
    let body = hyper::body::to_bytes(req.into_body()).await.unwrap();
    let name = String::from_utf8(body.to_vec()).unwrap();

    let res = channel_router.send_command(Message::ChannelCreate { name }, None).await;

    match res {
        Ok(CommandResponse::ChannelCreate(name)) => {
            (StatusCode::CREATED, name).into_response()
        }
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
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
    Path(id): Path<String>,
    Extension(channel_router): Extension<ChannelRouter>,
) -> impl IntoResponse {
    let res = channel_router.send_command(Message::ChannelGet { name: id.to_string() }, None).await;

    match res {
        Ok(CommandResponse::ChannelGet(channel)) => {
            if channel.is_none() {
                return (StatusCode::NOT_FOUND, "Channel not found".to_string()).into_response();
            }

            let body = serde_json::to_string(&channel.unwrap()).unwrap();
            (StatusCode::OK, body).into_response()
        }
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_channel(
    Path(id): Path<String>,
    Extension(channel_router): Extension<ChannelRouter>,
    req: Request<Body>,
) -> impl IntoResponse {
    // Your implementation here
}

async fn delete_channel(
    Path(id): Path<String>,
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
