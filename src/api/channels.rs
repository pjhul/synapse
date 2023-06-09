use axum::{
    body::Body,
    extract::{Extension, Path},
    http::Request,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use hyper::StatusCode;
use serde::Deserialize;

use crate::{
    auth::AuthConfig,
    channel::router::{ChannelRouter, CommandResponse},
    message::Message,
};

#[derive(Deserialize)]
struct CreateChannelBody {
    name: String,
    auth: Option<AuthConfig>,
    presence: Option<bool>,
}

async fn create_channel(
    Extension(channel_router): Extension<ChannelRouter>,
    req: Request<Body>,
) -> impl IntoResponse {
    let body = hyper::body::to_bytes(req.into_body()).await.unwrap();

    match serde_json::from_slice::<CreateChannelBody>(&body) {
        Ok(body) => {
            let res = channel_router
                .send_command(
                    Message::ChannelCreate {
                        name: body.name,
                        auth: body.auth,
                        presence: body.presence.unwrap_or(false),
                    },
                    None,
                )
                .await;

            match res {
                Ok(CommandResponse::ChannelCreate(name)) => {
                    (StatusCode::CREATED, name).into_response()
                }
                Ok(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                )
                    .into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn get_channels(Extension(channel_router): Extension<ChannelRouter>) -> impl IntoResponse {
    let res = channel_router
        .send_command(Message::ChannelGetAll, None)
        .await;

    match res {
        Ok(CommandResponse::ChannelGetAll(channels)) => {
            let body = serde_json::to_string(&channels).unwrap();
            (StatusCode::OK, body).into_response()
        }
        Ok(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn get_channel(
    Path(name): Path<String>,
    Extension(channel_router): Extension<ChannelRouter>,
) -> impl IntoResponse {
    let res = channel_router
        .send_command(
            Message::ChannelGet {
                name: name.to_string(),
            },
            None,
        )
        .await;

    match res {
        Ok(CommandResponse::ChannelGet(channel)) => {
            if channel.is_none() {
                return (StatusCode::NOT_FOUND, "Channel not found".to_string()).into_response();
            }

            let body = serde_json::to_string(&channel.unwrap()).unwrap();
            (StatusCode::OK, body).into_response()
        }
        Ok(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn update_channel(
    Path(_name): Path<String>,
    Extension(_channel_router): Extension<ChannelRouter>,
    _req: Request<Body>,
) -> impl IntoResponse {
    unimplemented!("There is nothing to update on a channel yet")
}

async fn delete_channel(
    Path(name): Path<String>,
    Extension(channel_router): Extension<ChannelRouter>,
) -> impl IntoResponse {
    let res = channel_router
        .send_command(
            Message::ChannelDelete {
                name: name.to_string(),
            },
            None,
        )
        .await;

    match res {
        Ok(CommandResponse::ChannelDelete(name)) => (StatusCode::OK, name).into_response(),
        Ok(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

pub fn channel_routes(channels: ChannelRouter) -> Router {
    Router::new()
        .route("/channels", post(create_channel).get(get_channels))
        .route(
            "/channels/:name",
            get(get_channel).put(update_channel).delete(delete_channel),
        )
        .layer(Extension(channels))
}
