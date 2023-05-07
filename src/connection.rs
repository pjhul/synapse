use std::{net::SocketAddr, result::Result};

use axum::extract::ws::{Message as WebSocketMessage, WebSocket};

use futures_util::future::select;
use futures_util::{pin_mut, StreamExt, TryStreamExt};
use log::{error, warn};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::{error::SendError, UnboundedSender};

use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::channel::router::ChannelRouter;
use crate::message::Message;
use crate::metrics::Metrics;

pub type ConnectionSender = UnboundedSender<WebSocketMessage>;

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
        let (sender, receiver) = unbounded_channel::<WebSocketMessage>();
        self.sender = Some(sender);

        self.increment("Connection::connections", 1);

        let (write, read) = ws.split();

        let self_clone = self.clone();

        let broadcast_incoming = read.try_for_each(|msg| {
            let self_clone = self_clone.clone();

            self_clone.increment("Connection::messages", 1);

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
                            .unwrap();
                    }
                } else {
                    warn!("Received a non-text message");
                }

                Ok(())
            }
        });

        // FIXME: We should have a periodic check that the connection is still alive

        let receiver = UnboundedReceiverStream::new(receiver);
        let forward_outgoing = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, forward_outgoing);
        select(broadcast_incoming, forward_outgoing).await;

        self.sender.take();

        self.decrement("Connection::connections", 1);

        Ok(())
    }

    pub fn send(&self, msg: WebSocketMessage) -> Result<(), SendError<WebSocketMessage>> {
        if let Some(ref sender) = self.sender {
            sender.send(msg)
        } else {
            warn!("Attempted to send a message to a connection that has no sender");
            Ok(())
        }
    }
}

impl Metrics for Connection {}
