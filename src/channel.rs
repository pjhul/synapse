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
    pub connections: HashMap<SocketAddr, Sender>,
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

    pub fn add_channel(&self, name: &String) {
        let mut channels = self.channels.lock().unwrap();

        if channels.contains_key(name) {
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
                connections: HashMap::new(),
            },
        );
    }

    pub fn add_connection(&self, channel_name: &String, addr: SocketAddr, sender: Sender) -> Result<(), String> {
        let mut channels = self.channels.lock().unwrap();

        if let Some(channel) = channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            if connections.contains_key(&addr) {
                info!("Connection already exists for {}", addr);
                return Err(format!("Connection already exists for {}", addr));
            }

            connections.insert(addr, sender);
            info!("Added connection for {}", addr);

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }

    pub fn remove_connection(&self, channel_name: &String, addr: SocketAddr) -> Result<(), String> {
        let mut channels = self.channels.lock().unwrap();

        if let Some(channel) = channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            connections.remove(&addr);
            info!("Removed connection for {}", addr);

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }

    pub fn remove_connection_from_all_channels(&self, addr: SocketAddr) {
        let mut channels = self.channels.lock().unwrap();

        for (_, channel) in channels.iter_mut() {
            let connections = &mut channel.connections;
            connections.remove(&addr);
        }

        info!("Removed connection for {}", addr);
    }

    pub fn has_channel(&self, channel_name: String) -> bool {
        let channels = self.channels.lock().unwrap();

        channels.contains_key(&channel_name)
    }

    pub fn broadcast(&self, channel_name: &String, skip_addr: SocketAddr, message: Message) -> Result<(), String> {
        let channels = self.channels.lock().unwrap();

        if let Some(channel) = channels.get(channel_name) {
            let connections = &channel.connections;

            for (addr, sender) in connections.iter() {
                if *addr == skip_addr {
                    continue;
                }

                sender.unbounded_send(message.clone()).unwrap();
            }

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }
}
