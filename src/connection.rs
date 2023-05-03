use std::{net::SocketAddr, result::Result};

use axum::extract::ws::{Message as WebSocketMessage, WebSocket};

use futures_util::future::{select, self};
use futures_util::stream::{Forward, SplitSink};
use futures_util::{pin_mut, Future, StreamExt, TryFuture, TryStreamExt};
use log::warn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::{error::SendError, UnboundedSender};

use tokio_stream::wrappers::UnboundedReceiverStream;

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

    pub async fn listen<F>(&mut self, ws: WebSocket, mut f: F) -> Result<(), ()>
    where
        F: FnMut(Message) -> Fut + Send + 'static,
    {
        let (write, read) = ws.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            if let Ok(msg) = msg.to_text() {
                /*let msg = match msg.parse::<Message>() {
                    Ok(msg) => msg,
                    Err(e) => {
                        warn!("Received an invalid message: {}", e);

                        /*self.send(Message::Error {
                            message: format!("Invalid message: {}", e),
                        }.into());*/


                    }
                };*/

                let msg = msg.parse::<Message>().unwrap();

                // Need to somehow ensure that this doesn't force the entire method return type to
                // be Fut and not just something that imlements future
                return f(msg);
            } else {
                warn!("Received a non-text message");
            }

            future::ok(())
        });

        pin_mut!(broadcast_incoming /*, receive_from_others*/);
        select(broadcast_incoming, self.forward(write)).await;

        Ok(())
    }

    pub fn forward(
        &mut self,
        socket_writer: SplitSink<WebSocket, WebSocketMessage>,
    ) -> impl Future<Output = Result<(), axum::Error>> {
        let (sender, mut receiver) = unbounded_channel::<WebSocketMessage>();

        self.sender = Some(sender);

        let receiver = UnboundedReceiverStream::from(receiver);
        receiver.map(|msg| Ok(msg)).forward(socket_writer)
    }

    pub fn send(&self, msg: WebSocketMessage) -> Result<(), SendError<WebSocketMessage>> {
        if let Some(sender) = self.sender {
            sender.send(msg)
        } else {
            Ok(())
        }
    }
}
