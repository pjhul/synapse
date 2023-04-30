use std::{
    env,
    io::Error,
};

use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use log::{info, warn};
use serde::Deserialize;
use tokio::net::{TcpListener, TcpStream};

use synapse::channel::ChannelMap;
use tokio_tungstenite::tungstenite::Message;

#[derive(Deserialize)]
struct MessageBody {
    channel: String,
    payload: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::init_from_env(env);

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let channels = ChannelMap::new();

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(channels.clone(), stream));
    }

    Ok(())
}

async fn accept_connection(channels: ChannelMap, stream: TcpStream) {
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

    // TODO: Remove connection from all channels
    // connections.lock().unwrap().remove(&addr);
}
