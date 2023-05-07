use std::net::SocketAddr;

use axum::extract::ws::Message;
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use synapse::{channel::ChannelMap, connection::Connection};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

// Helper function to create a SocketAddr
fn create_socket_addr() -> SocketAddr {
    "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
}

// Helper function to create a Sender<Message>
fn create_sender() -> UnboundedSender<Message> {
    let (_tx, _rx) = unbounded_channel();
    _tx
}

fn create_connection() -> Connection {
    Connection {
        addr: create_socket_addr(),
        sender: Some(create_sender()),
    }
}

fn create_channel_map() -> ChannelMap {
    let (_tx, rx) = mpsc::channel(1);
    ChannelMap::new(rx)
}

#[tokio::test]
async fn test_add_channel() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");

    channel_map.add_channel(&channel_name);

    assert!(channel_map.has_channel(channel_name));
}

#[tokio::test]
async fn test_add_connection() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");
    let conn = create_connection();

    channel_map.add_channel(&channel_name);
    channel_map.add_connection(&channel_name, conn.clone());

    let channels = channel_map.channels;
    let channel = channels.get(&channel_name).unwrap();
    let connections = &channel.connections;

    assert!(connections.contains_key(&conn.addr));
}

#[tokio::test]
async fn test_remove_connection() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");
    let conn = create_connection();

    channel_map.add_channel(&channel_name);
    channel_map.add_connection(&channel_name, conn.clone());

    channel_map.remove_connection(&channel_name, conn.addr);

    let channels = channel_map.channels;
    let channel = channels.get(&channel_name).unwrap();
    let connections = &channel.connections;

    assert!(!connections.contains_key(&conn.addr));
}

#[tokio::test]
async fn test_remove_connection_from_all() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");
    let conn = create_connection();

    channel_map.add_channel(&channel_name);
    channel_map.add_connection(&channel_name, conn.clone());

    channel_map.remove_connection_from_all(conn.addr);

    let channels = channel_map.channels;
    let channel = channels.get(&channel_name).unwrap();
    let connections = &channel.connections;

    assert!(!connections.contains_key(&conn.addr));
}

#[tokio::test]
async fn test_has_channel() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");

    assert!(!channel_map.has_channel(channel_name.clone()));

    channel_map.add_channel(&channel_name);

    assert!(channel_map.has_channel(channel_name));
}

#[tokio::test]
async fn test_broadcast() {
    let mut channel_map = create_channel_map();
    let channel_name = String::from("test_channel");

    let (sender1, receiver1) = unbounded_channel();
    let (sender2, receiver2) = unbounded_channel();

    let conn1 = Connection {
        addr: "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        sender: Some(sender1),
    };

    let conn2 = Connection {
        addr: "127.0.0.2:8080".parse::<SocketAddr>().unwrap(),
        sender: Some(sender2),
    };

    let skip_addr = conn1.addr;

    channel_map.add_channel(&channel_name);
    channel_map.add_connection(&channel_name, conn1);
    channel_map.add_connection(&channel_name, conn2);

    let msg_text = "Hello, world!";
    let message = Message::Text(msg_text.to_owned());
    channel_map.broadcast(&channel_name, skip_addr, message.clone());

    let mut receiver1 = UnboundedReceiverStream::new(receiver1);
    let mut receiver2 = UnboundedReceiverStream::new(receiver2);

    assert!(receiver1.next().now_or_never().is_none());
    let received_msg = receiver2.next().await.unwrap();
    assert_eq!(received_msg, message)
}
