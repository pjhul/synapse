use std::{net::SocketAddr, sync::Arc};

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
use log::{info, warn, error};

use crate::channel::ChannelMap;
use crate::message::Message;

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
                Self::handle_connection(socket, addr, channels)
                    .await
                    .unwrap();
            });
        })
    }

    async fn handle_connection(
        stream: WebSocket,
        addr: SocketAddr,
        channels: Arc<ChannelMap>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = futures_channel::mpsc::unbounded::<WebSocketMessage>();

        let (write, read) = stream.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            if let Ok(msg) = msg.to_text() {
                let msg = match msg.parse::<Message>() {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!("Received an invalid message: {}", e);

                        let error_msg = Message::Error {
                            message: format!("Invalid message: {}", e),
                        };

                        sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();

                        return future::ready(Ok(()));
                    }
                };

                // FIXME: This is messy, we should branch and encapsulate this logic better
                match msg {
                    Message::Join { ref channel } => {
                        channels.add_channel(channel);

                        if let Err(e) = channels.add_connection(channel, addr, sender.clone()) {
                            // TODO: Package this up into a helper function
                            error!("Failed to add connection: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to add connection: {}", e),
                            };

                            sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();
                        }
                    }
                    Message::Leave { ref channel } => {
                        if let Err(e) = channels.remove_connection(channel, addr) {
                            // TODO: Package this up into a helper function
                            error!("Failed to remove connection: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to remove connection: {}", e),
                            };

                            sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();
                        }
                    }
                    Message::Broadcast { ref channel, body } => {
                        let msg = Message::Broadcast {
                            channel: channel.clone(),
                            body,
                        };

                        if let Err(e) = channels.broadcast(channel, addr, WebSocketMessage::Text(msg.into())) {
                            // TODO: Package this up into a helper function
                            error!("Failed to broadcast message: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to broadcast message: {}", e),
                            };

                            sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();
                        }
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                    }
                }
            } else {
                warn!("Received a non-text message");
            }

            future::ready(Ok(()))
        });

        let receive_from_others = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;

        channels.remove_connection_from_all_channels(addr);

        Ok(())
    }
}
