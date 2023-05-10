use std::{net::SocketAddr, result::Result};

use axum::Error;
use axum::extract::ws::{Message as WebSocketMessage, WebSocket};

use futures_util::future::select;
use futures_util::stream::SplitSink;
use futures_util::{pin_mut, StreamExt, TryStreamExt, SinkExt};
use log::{error, info, warn};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::{channel, error::SendError, Sender};
use tokio_tungstenite::{
    tungstenite::Error as TungsteniteError,
};

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
        let (sender, receiver) = channel::<WebSocketMessage>(512);
        self.sender = Some(sender);

        self.increment_active_connections();

        let (write, read) = ws.split();

        let self_clone = self.clone();

        let broadcast_incoming = read.try_for_each(|msg| {
            let self_clone = self_clone.clone();

            self.increment_messages_received();

            async move {
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

                            return Ok(());
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

                Ok(())
            }
        });

        // FIXME: We should have a periodic check that the connection is still alive
        async {
            let forward_outgoing = self.forward_messages(receiver, write);

            pin_mut!(broadcast_incoming, forward_outgoing);
            select(broadcast_incoming, forward_outgoing).await;
        }.await;

        self.sender.take();

        self.decrement_active_connections();

        info!("Connection closed");

        Ok(())
    }

    async fn forward_messages(&self, receiver: Receiver<WebSocketMessage>, mut writer: SplitSink<WebSocket, WebSocketMessage>) {
        let receiver = ReceiverStream::new(receiver);

        let mut receiver = receiver.chunks(16);

        // TODO: Consider batching reads and writes here
        while let Some(msgs) = receiver.next().await {
            for msg in msgs {
                self.increment_messages_sent(1);

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
            }

            writer.flush().await.unwrap();
        }
    }

    pub async fn send(&self, msg: WebSocketMessage) -> Result<(), SendError<WebSocketMessage>> {
        if let Some(ref sender) = self.sender {
            sender.send(msg).await
        } else {
            warn!("Attempted to send a message to a connection that has no sender");
            Ok(())
        }
    }
}

impl Metrics for Connection {}
