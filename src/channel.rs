use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

type Sender = UnboundedSender<Message>;

pub struct Channel {
    pub name: String,
    connections: Arc<Mutex<HashMap<SocketAddr, Sender>>>,
}

pub struct ChannelMap {
    pub channels: Arc<Mutex<HashMap<String, Channel>>>,
}

impl ChannelMap {
    pub fn new() -> Self {
        ChannelMap {
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_channel(&self, name: String) {
        let mut channels = self.channels.lock().unwrap();
        channels.insert(name.clone(), Channel {
            name,
            connections: Arc::new(Mutex::new(HashMap::new())),
        });
    }

    pub fn add_connection(&self, channel_name: String, addr: SocketAddr, sender: Sender) {
        let mut channels = self.channels.lock().unwrap();
        let channel = channels.get_mut(&channel_name).unwrap();
        channel.connections.lock().unwrap().insert(addr, sender);
    }
}

impl Clone for ChannelMap {
    fn clone(&self) -> Self {
        ChannelMap {
            channels: self.channels.clone(),
        }
    }
}
