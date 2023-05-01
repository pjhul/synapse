use std::{convert::Infallible, net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures::future::IntoStream;
use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use hyper::{
    server::conn::AddrStream,
    service::{make_service_fn, service_fn, Service},
    Body, Request,
};
use log::{info, warn};
use serde::Deserialize;
use tokio::net::{TcpListener, TcpStream};

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

        let app = Router::new().route("/ws", get(Self::ws_handler));

        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ) -> impl IntoResponse {
        info!("New connection from: {}", addr);

        ws.on_upgrade(move |socket| async move {
            // let channels = self.channels.clone();
            let channels = Arc::new(ChannelMap::new());

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
