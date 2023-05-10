use std::{net::SocketAddr, result::Result};

use axum::extract::ws::{Message as WebSocketMessage, WebSocket};
use axum::Error;

use futures_util::future::select;
use futures_util::stream::SplitSink;
use futures_util::{pin_mut, SinkExt, StreamExt, TryStreamExt};
use log::{error, info, warn};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::{channel, error::SendError, Sender};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Error as TungsteniteError;

use tokio_stream::wrappers::ReceiverStream;

use crate::channel::router::{ChannelRouter, CommandResponse};
use crate::message::Message;
use crate::metrics::Metrics;

pub type ConnectionSender = Sender<WebSocketMessage>;

#[derive(Clone, Debug)]
pub struct Connection {
    pub id: String,
    pub addr: SocketAddr,
    // The lifetime of the connection should mirror the lifetime of the websocket itself
    // pub stream: &'a mut WebSocket,
    pub sender: Option<ConnectionSender>,
    // TODO: Store authorization information here
}

impl Connection {
    pub fn new(id: Option<String>, addr: SocketAddr) -> Self {
        let id = id
            .or_else(|| Some(uuid::Uuid::new_v4().to_string()))
            .unwrap();

        Self {
            id,
            addr,
            sender: None,
        }
    }

    /**
     * Listens for messages on the websocket and forwards them to the channel router. Returns a
     * future that completes when the websocket is closed.
     */
    pub async fn listen(
        &mut self,
        ws: WebSocket,
        channels: &ChannelRouter,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = channel::<WebSocketMessage>(32);
        self.sender = Some(sender);

        Self::increment_active_connections();

        let (write, mut read) = ws.split();

        let self_clone = self.clone();

        let channels = channels.clone();

        let broadcast_incoming: JoinHandle<Result<(), String>> = tokio::spawn(async move {
            while let Some(msg) = read.try_next().await.unwrap() {
                let self_clone = self_clone.clone();

                Self::increment_messages_received();

                if let Ok(msg) = msg.to_text() {
                    let msg = match msg.parse::<Message>() {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Received an invalid message: {}", e);

                            self_clone
                                .send(
                                    Message::Error {
                                        message: format!("Invalid message: {}", e),
                                    }
                                    .into(),
                                )
                                .await
                                .unwrap();

                            continue;
                        }
                    };

                    let result = channels
                        .send_command(msg.clone(), Some(self_clone.clone()))
                        .await;

                    if let Err(e) = result {
                        error!("{}", e);

                        self_clone
                            .send(Message::Error { message: e }.into())
                            .await
                            .unwrap();
                    } else {
                        let result = result.unwrap();

                        if let CommandResponse::Unauthorized(msg) = result {
                            self_clone
                                .send(Message::Error { message: msg }.into())
                                .await
                                .unwrap();
                        }
                    }
                } else {
                    warn!("Received a non-text message");
                }
            }

            Ok(())
        });

        // FIXME: We should have a periodic check that the connection is still alive
        let forward_outgoing = self.forward_messages(receiver, write);

        tokio::select! {
            _ = forward_outgoing => {},
            _ = broadcast_incoming => {},
        };

        self.sender.take();

        Self::decrement_active_connections();

        // self.decrement_active_connections();

        info!("Connection closed");

        Ok(())
    }

    fn forward_messages(
        &self,
        receiver: Receiver<WebSocketMessage>,
        mut writer: SplitSink<WebSocket, WebSocketMessage>,
    ) -> tokio::task::JoinHandle<()> {
        let mut receiver = ReceiverStream::new(receiver);

        // let mut receiver = receiver.chunks(8);

        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                // info!("Received {} messages", msgs.len());
                // Self::increment_messages_sent(msgs.len() as u64);

                Self::decrement_connection_buffer_size();
                Self::increment_messages_sent(1);

                // for msg in msgs {
                let msg = msg.into();
                let result = writer.send(msg).await;

                if let Err(e) = result {
                    let inner_error = e.into_inner();

                    let err = inner_error.downcast::<TungsteniteError>();

                    if let Ok(err) = err {
                        match *err {
                            TungsteniteError::ConnectionClosed => {
                                warn!("Connection has already been closed");
                                // TODO: Stop listening when the connection is closed
                                return;
                            }
                            // FIXME: We shouldn't close the connection on every error
                            _ => {
                                error!("Error sending message: {}", err);
                                return;
                            }
                        }
                    } else {
                        error!("Received non-websocket error");
                    }
                }

                writer.flush().await.unwrap();
            }
            // }

            // writer.flush().await.unwrap();
        })
    }

    pub async fn send(&self, msg: WebSocketMessage) -> Result<(), SendError<WebSocketMessage>> {
        Self::increment_connection_buffer_size();

        if let Some(ref sender) = self.sender {
            sender.send(msg).await
        } else {
            warn!("Attempted to send a message to a connection that has no sender");
            Ok(())
        }
    }
}

impl Metrics for Connection {}
