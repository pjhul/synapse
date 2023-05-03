use std::net::SocketAddr;

use axum::{
    extract::{
        ws::{Message as WebSocketMessage, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};

use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use log::{error, info, warn};
use tokio::sync::mpsc;

use crate::channel::{Command, ChannelRouter};
use crate::message::Message;

pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(self, addr: &str) {
        info!("Listening on: {}", addr);

        let (tx, mut rx) = mpsc::channel::<Command>(1024);
        let channels = ChannelRouter::new(tx, rx);

        let app = Router::new().route(
            "/ws",
            get(
                move |ws: WebSocketUpgrade, conn_info: ConnectInfo<SocketAddr>| {
                    Self::ws_handler(ws, conn_info, channels)
                },
            ),
        );


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
        channels: ChannelRouter
    ) -> impl IntoResponse {
        info!("New connection from: {}", addr);
        let channels = channels.clone();

        ws.on_upgrade(move |socket| async move {
            tokio::spawn(async move {
                let addr = addr.to_owned();
                Self::handle_connection(socket, addr, channels).await.unwrap();
            });
        })
    }

    async fn handle_connection(
        stream: WebSocket,
        addr: SocketAddr,
        channels: ChannelRouter
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = futures_channel::mpsc::unbounded::<WebSocketMessage>();

        let (write, read) = stream.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            let sender = sender.clone();
            let channels = channels.clone();

            async move {
                if let Ok(msg) = msg.to_text() {
                    let msg = match msg.parse::<Message>() {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Received an invalid message: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Invalid message: {}", e),
                            };

                            sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();

                            return Ok(());
                        }
                    };

                    channels.send_command(msg, sender, addr).await;
                } else {
                    warn!("Received a non-text message");
                }

                Ok(())
            }
        });

        let receive_from_others = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;

        channels.send_command(Message::Disconnect, sender, addr).await;

        Ok(())
    }
}
