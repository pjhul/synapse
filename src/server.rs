use std::sync::Arc;

use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use log::{info, warn};
use serde::Deserialize;
use tokio::net::{TcpListener, TcpStream};

use crate::channel::ChannelMap;
use tokio_tungstenite::tungstenite::Message;

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
        let try_socket = TcpListener::bind(&addr).await;
        let listener = try_socket.expect("Failed to bind");
        info!("Listening on: {}", addr);

        while let Ok((stream, _)) = listener.accept().await {
            let channels = self.channels.clone();

            tokio::spawn(async move {
                Server::accept_connection(channels, stream).await;
            });
        }
    }

    async fn accept_connection(channels: Arc<ChannelMap>, stream: TcpStream) {
        let addr = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        info!("Incoming connection: {}", addr);

        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .expect("Error during the websocket handshake occurred");

        let (sender, receiver) = futures_channel::mpsc::unbounded();

        let (write, read) = ws_stream.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            info!("Received a message from {}: {}", addr, msg);

            if msg.is_text() {
                let msg = msg.to_text().unwrap();

                // TODO: Handle error better
                let body: MessageBody = serde_json::from_str(msg).unwrap();

                channels.add_channel(body.channel.clone());
                channels.add_connection(body.channel.clone(), addr, sender.clone());
                channels.broadcast(body.channel.clone(), addr, Message::text(msg));
            } else {
                warn!("Received a non-text message from {}: {}", addr, msg);
            }

            future::ok(())
        });

        let receive_from_others = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;

        channels.remove_connection(addr);
    }
}
