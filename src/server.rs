use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};

use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use log::{info, warn};
use serde::Deserialize;

use crate::channel::ChannelMap;

#[derive(Deserialize)]
struct MessageBody {
    channel: String,
    payload: serde_json::Value,
}

pub struct Server {
    channels: Arc<ChannelMap>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            channels: Arc::new(ChannelMap::new()),
        }
    }

    pub async fn run(self, addr: &str) {
        info!("Listening on: {}", addr);

        let app = Router::new().route(
            "/ws",
            get(
                move |ws: WebSocketUpgrade, conn_info: ConnectInfo<SocketAddr>| {
                    Self::ws_handler(ws, conn_info, self.channels)
                },
            ),
        );

        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        channels: Arc<ChannelMap>,
    ) -> impl IntoResponse {
        info!("New connection from: {}", addr);
        let channels = channels.clone();

        ws.on_upgrade(move |socket| async move {
            tokio::spawn(async move {
                let addr = addr.to_owned();
                Server::accept_connection(socket, addr, channels)
                    .await
                    .unwrap();
            });
        })
    }

    async fn accept_connection(
        stream: WebSocket,
        addr: SocketAddr,
        channels: Arc<ChannelMap>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = futures_channel::mpsc::unbounded::<axum::extract::ws::Message>();

        let (write, read) = stream.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            if let Ok(msg) = msg.to_text() {
                // TODO: Handle error better
                let body: MessageBody = serde_json::from_str(msg).unwrap();

                channels.add_channel(body.channel.clone());
                channels.add_connection(body.channel.clone(), addr, sender.clone());
                channels.broadcast(body.channel.clone(), addr, Message::Text(msg.into()));
            } else {
                warn!("Received a non-text message");
            }

            future::ready(Ok(()))
        });

        let receive_from_others = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;

        channels.remove_connection(addr);

        Ok(())
    }
}
