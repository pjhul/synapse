use std::{net::SocketAddr, result::Result};

use axum::extract::ws::{Message as WebSocketMessage, WebSocket};

use futures_util::future::select;
use futures_util::{pin_mut, StreamExt, TryStreamExt};
use log::{warn, info};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::{error::SendError, UnboundedSender};

use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::channel::ChannelRouter;
use crate::message::Message;

pub type ConnectionSender = UnboundedSender<WebSocketMessage>;

#[derive(Clone, Debug)]
pub struct Connection {
    pub addr: SocketAddr,
    sender: Option<ConnectionSender>,
}

impl Connection {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr, sender: None }
    }

    /**
     * Listens for messages on the websocket and forwards them to the channel router. Returns a
     * future that completes when the websocket is closed.
     */
    pub async fn listen(&mut self, ws: WebSocket, channels: &ChannelRouter) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = unbounded_channel::<WebSocketMessage>();
        self.sender = Some(sender);

        let (write, read) = ws.split();

        let self_clone = self.clone();

        let broadcast_incoming = read.try_for_each(|msg| {
            let self_clone = self_clone.clone();

            async move {
                if let Ok(msg) = msg.to_text() {
                    let msg = match msg.parse::<Message>() {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Received an invalid message: {}", e);

                            self_clone.send(
                                Message::Error {
                                    message: format!("Invalid message: {}", e),
                                }
                                .into(),
                            ).unwrap();

                            return Ok(());
                        }
                    };

                    channels.send_command(msg, self_clone).await;
                } else {
                    warn!("Received a non-text message");
                }

                Ok(())
            }
        });

        let receiver = UnboundedReceiverStream::new(receiver);
        let forward_outgoing = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, forward_outgoing);
        select(broadcast_incoming, forward_outgoing).await;

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
