use std::net::SocketAddr;

use futures::{FutureExt, StreamExt};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

use synapse::channel::ChannelMap;

// Helper function to create a SocketAddr
fn create_socket_addr() -> SocketAddr {
    "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
}

// Helper function to create a Sender<Message>
fn create_sender() -> UnboundedSender<Message> {
    let (_tx, _rx) = unbounded();
    _tx
}

#[tokio::test]
async fn test_add_channel() {
    let channel_map = ChannelMap::new();
    let channel_name = String::from("test_channel");

    channel_map.add_channel(channel_name.clone());

    assert!(channel_map.has_channel(channel_name));
}

#[tokio::test]
async fn test_add_connection() {
    let channel_map = ChannelMap::new();
    let channel_name = String::from("test_channel");
    let addr = create_socket_addr();
    let sender = create_sender();

    channel_map.add_channel(channel_name.clone());
    channel_map.add_connection(channel_name.clone(), addr, sender);

    let channels = channel_map.channels.lock().unwrap();
    let channel = channels.get(&channel_name).unwrap();
    let connections = channel.connections.lock().unwrap();

    assert!(connections.contains_key(&addr));
}

#[tokio::test]
async fn test_remove_connection() {
    let channel_map = ChannelMap::new();
    let channel_name = String::from("test_channel");
    let addr = create_socket_addr();
    let sender = create_sender();

    channel_map.add_channel(channel_name.clone());
    channel_map.add_connection(channel_name.clone(), addr, sender);

    channel_map.remove_connection(addr);

    let channels = channel_map.channels.lock().unwrap();
    let channel = channels.get(&channel_name).unwrap();
    let connections = channel.connections.lock().unwrap();

    assert!(!connections.contains_key(&addr));
}

#[tokio::test]
async fn test_has_channel() {
    let channel_map = ChannelMap::new();
    let channel_name = String::from("test_channel");

    assert!(!channel_map.has_channel(channel_name.clone()));

    channel_map.add_channel(channel_name.clone());

    assert!(channel_map.has_channel(channel_name));
}

#[tokio::test]
async fn test_broadcast() {
    let channel_map = ChannelMap::new();
    let channel_name = String::from("test_channel");
    let addr1 = create_socket_addr();
    let addr2 = "127.0.0.1:8081".parse().unwrap();
    let (sender1, mut receiver1) = unbounded();
    let (sender2, mut receiver2) = unbounded();
    let skip_addr = addr1;

    channel_map.add_channel(channel_name.clone());
    channel_map.add_connection(channel_name.clone(), addr1, sender1);
    channel_map.add_connection(channel_name.clone(), addr2, sender2);

    let msg_text = "Hello, world!";
    let message = Message::Text(msg_text.to_owned());
    channel_map.broadcast(channel_name.clone(), skip_addr, message.clone());

    assert!(receiver1.next().now_or_never().is_none());
    let received_msg = receiver2.next().await.unwrap();
    assert_eq!(received_msg, message)
}
