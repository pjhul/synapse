use std::{env, io::Error, sync::{Arc, Mutex}, collections::HashMap, net::SocketAddr};

use futures_channel::mpsc::UnboundedSender;
use futures_util::{future, StreamExt, TryStreamExt, pin_mut};
use log::info;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

type Sender = UnboundedSender<Message>;
type ConnectionMap = Arc<Mutex<HashMap<SocketAddr, Sender>>>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::init_from_env(env);

    let addr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let connections = ConnectionMap::new(Mutex::new(HashMap::new()));

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(connections.clone(), stream));
    }

    Ok(())
}

async fn accept_connection(connections: ConnectionMap, stream: TcpStream) {
    let addr = stream.peer_addr().expect("connected streams should have a peer address");
    info!("Incoming connection: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    let (sender, receiver) = futures_channel::mpsc::unbounded();
    connections.lock().unwrap().insert(addr, sender);

    let (write, read) = ws_stream.split();

    let broadcast_incoming = read.try_for_each(|msg| {
        info!("Received a message from {}: {}", addr, msg);

        let mut connections = connections.lock().unwrap();
        let iter = connections.iter_mut().filter(|(conn_addr, _)| **conn_addr != addr);

        for (_, tx) in iter {
            tx.unbounded_send(msg.clone()).unwrap();
        }

        future::ok(())
    });

    let receive_from_others = receiver.map(Ok).forward(write);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    connections.lock().unwrap().remove(&addr);
}
