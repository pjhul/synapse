use std::{net::SocketAddr, result::Result};

use axum::extract::ws::{Message as WebSocketMessage, WebSocket};

use futures_util::future::select;
use futures_util::stream::SplitSink;
use futures_util::{pin_mut, Future, StreamExt, TryStreamExt};
use log::warn;
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

        pin_mut!(broadcast_incoming);
        select(broadcast_incoming, self.forward(write)).await;

        Ok(())
    }

    pub fn forward(
        &mut self,
        socket_writer: SplitSink<WebSocket, WebSocketMessage>,
    ) -> impl Future<Output = Result<(), axum::Error>> {
        let (sender, receiver) = unbounded_channel::<WebSocketMessage>();

        self.sender = Some(sender);

        let receiver = UnboundedReceiverStream::from(receiver);
        receiver.map(|msg| Ok(msg)).forward(socket_writer)
    }

    pub fn send(&self, msg: WebSocketMessage) -> Result<(), SendError<WebSocketMessage>> {
        if let Some(ref sender) = self.sender {
            sender.send(msg)
        } else {
            Ok(())
        }
    }
}
