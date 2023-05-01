use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures_channel::mpsc::UnboundedSender;
use log::info;
use axum::extract::ws::Message;

type Sender = UnboundedSender<Message>;

#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: Arc<Mutex<HashMap<SocketAddr, Sender>>>,
}

#[derive(Clone, Debug)]
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

        if channels.contains_key(&name) {
            info!("Channel {} already exists", name);
            return;
        }

        // NOTE: There seems to be weird behavior around inserting a key into a map that is
        // contained in an Arc<Mutex<>>. The key is inserted, but it seems the map itself is copied
        // in some way? The issue was fixed by checking if the key existed first.
        channels.insert(
            name.clone(),
            Channel {
                name: name.clone(),
                connections: Arc::new(Mutex::new(HashMap::new())),
            },
        );
    }

    pub fn add_connection(&self, channel_name: String, addr: SocketAddr, sender: Sender) {
        let mut channels = self.channels.lock().unwrap();

        let channel = channels.get_mut(&channel_name).unwrap();
        let mut connections = channel.connections.lock().unwrap();

        if connections.contains_key(&addr) {
            info!("Connection already exists for {}", addr);
            return;
        }

        connections.insert(addr, sender);
    }

    pub fn remove_connection(&self, addr: SocketAddr) {
        let mut channels = self.channels.lock().unwrap();

        for (_, channel) in channels.iter_mut() {
            let mut connections = channel.connections.lock().unwrap();
            connections.remove(&addr);
        }

        info!("Removed connection for {}", addr);
    }

    pub fn has_channel(&self, channel_name: String) -> bool {
        let channels = self.channels.lock().unwrap();

        channels.contains_key(&channel_name)
    }

    pub fn broadcast(&self, channel_name: String, skip_addr: SocketAddr, message: Message) {
        let channels = self.channels.lock().unwrap();

        let channel = channels.get(&channel_name).unwrap();
        let connections = channel.connections.lock().unwrap();

        for (addr, sender) in connections.iter() {
            if *addr == skip_addr {
                continue;
            }

            sender.unbounded_send(message.clone()).unwrap();
        }
    }
}
