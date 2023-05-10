use std::net::SocketAddr;

use axum::{extract::ConnectInfo, response::IntoResponse, routing::get, Router};

use hyper::StatusCode;
use log::info;
use prometheus::{Encoder, TextEncoder};
use tokio::sync::mpsc;

use crate::message::Message;
use crate::metrics::Metrics;
use crate::ws::upgrade::{WebSocket, WebSocketUpgrade};
use crate::{api::channels::channel_routes, metrics::get_metrics};
use crate::{
    channel::router::{ChannelRouter, Command},
    connection::Connection,
};

pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(self, addr: &str) {
        info!("Listening on: {}", addr);

        let (tx, rx) = mpsc::channel::<Command>(256);
        let channels = ChannelRouter::new(tx, rx);

        let channel_router = channel_routes(channels.clone());

        let ws_router = Router::new().route(
            "/ws",
            get(
                move |ws: WebSocketUpgrade, conn_info: ConnectInfo<SocketAddr>| {
                    Self::ws_handler(ws, conn_info, channels)
                },
            ),
        );

        let app = Router::new()
            .route("/metrics", get(Self::metrics_handler))
            .nest("/api", channel_router)
            .nest("/", ws_router);

        self.run_server(app, addr).await
    }

    async fn run_server(self, app: Router, addr: &str) {
        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        channels: ChannelRouter,
    ) -> impl IntoResponse {
        info!("New connection from: {}", addr);

        ws.on_upgrade(move |socket| async move {
            tokio::spawn(async move {
                let addr = addr.to_owned();
                Self::handle_connection(socket, addr, channels)
                    .await
                    .unwrap();
            });
        })
    }

    async fn handle_connection(
        stream: WebSocket,
        addr: SocketAddr,
        channels: ChannelRouter,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = Connection::new(None, addr);

        conn.listen(stream, &channels).await?;

        channels
            .send_command(Message::Disconnect, Some(conn))
            .await
            .unwrap();

        Ok(())
    }

    async fn metrics_handler() -> impl IntoResponse {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metrics = get_metrics().unwrap();
        encoder.encode(&metrics, &mut buffer).unwrap();

        (StatusCode::OK, buffer)
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics for Server {}
