use std::{collections::HashMap, net::SocketAddr};

use axum::extract::ws::Message as WebSocketMessage;
use log::{info, warn};
use serde_json::Value;
use tokio::sync::mpsc::Receiver;

use crate::connection::Connection;
use crate::message::Message;

use super::router::Command;

#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: HashMap<SocketAddr, Connection>,
}

#[derive(Debug)]
pub struct ChannelStore {
    receiver: Receiver<Command>,
    // TODO: When we switch back to a mutex, should probably restructure this into a sharded Mutex, and potentially use an RwLock
    // instead as well
    // See: https://docs.rs/dashmap/latest/dashmap/
    pub channels: HashMap<String, Channel>,
}

impl ChannelStore {
    pub fn new(rx: Receiver<Command>) -> Self {
        Self {
            receiver: rx,
            channels: HashMap::new(),
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            while let Some(cmd) = self.receiver.recv().await {
                let Command { msg, conn, result } = cmd;

                let cmd_result = match msg.clone() {
                    Message::Join {
                        ref channel,
                        presence,
                    } => {
                        self.handle_join(channel, conn.clone())
                    }
                    Message::Leave { ref channel } => {
                        self.handle_leave(channel, conn.clone())
                    }
                    Message::Disconnect => {
                        self.handle_disconnect(conn.clone())
                    }
                    Message::Broadcast { ref channel, body } => {
                        self.handle_broadcast(channel, body, conn.clone())
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                        Ok(())
                    }
                    _ => {
                        Err(format!("Received an invalid message: {:?}", msg))
                    }
                };

                if let Some(result) = result {
                    result.send(cmd_result).unwrap();
                }
            }
        });
    }

    // Message handlers

    fn handle_join(&mut self, channel: &String, conn: Connection) -> Result<(), String> {
        // TODO: Remove this, eventually all channels will have to be created first
        self.add_channel(channel);

        self.add_connection(channel, conn)?;

        let channel = self.get_channel(channel).unwrap().clone();

        self.broadcast(
            &channel.name,
            WebSocketMessage::Text(
                Message::Presence {
                    channel: channel.name.clone(),
                    connections: channel
                        .connections
                        .values()
                        .map(|c| c.addr.to_string())
                        .collect(),
                }
                .into(),
            ),
            None,
        )
    }

    fn handle_leave(&mut self, channel: &String, conn: Connection) -> Result<(), String> {
        self.remove_connection(channel, conn.addr)
    }

    fn handle_disconnect(&mut self, conn: Connection) -> Result<(), String> {
        self.remove_connection_from_all(conn.addr)
    }

    fn handle_broadcast(&mut self, channel: &String, body: Value, conn: Connection) -> Result<(), String> {
        let msg = Message::Broadcast {
            channel: channel.clone(),
            body,
        };

        self.broadcast(channel, msg.into(), Some(conn.addr))
    }

    // Channel API handlers

    fn handle_channel_get_all(&self) -> Result<Vec<String>, String> {
        Ok(self.channels.keys().cloned().collect())
    }

    // Internal API

    fn get_channel(&self, name: &String) -> Option<&Channel> {
        self.channels.get(name)
    }

    fn add_channel(&mut self, name: &String) {
        let channels = &mut self.channels;

        if channels.contains_key(name) {
            info!("Channel {} already exists", name);
            return;
        }

        channels.insert(
            name.clone(),
            Channel {
                name: name.clone(),
                connections: HashMap::new(),
            },
        );
    }

    fn add_connection(
        &mut self,
        channel_name: &String,
        conn: Connection,
    ) -> Result<(), String> {
        if let Some(channel) = self.channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            if connections.contains_key(&conn.addr) {
                info!("Connection already exists for {}", conn.addr);
                return Err(format!("Connection already exists for {}", conn.addr));
            }

            info!("Added connection for {}", conn.addr);
            connections.insert(conn.addr, conn);

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }

    fn remove_connection(
        &mut self,
        channel_name: &String,
        addr: SocketAddr,
    ) -> Result<(), String> {
        let channel = self.channels.get_mut(channel_name);

        if let Some(channel) = channel {
            channel.connections.remove(&addr);
            info!("Removed connection for {}", addr);

            // if connections.len() == 0 {
            //     self.channels.remove(&channel.name);
            // }
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }

        // FIXME: Don't `get` the channel twice, use the same one from above
        let channel = self.channels.get(channel_name);

        let msg = Message::Presence {
            channel: channel_name.clone(),
            connections: channel
                .unwrap()
                .connections
                .values()
                .map(|c| c.addr.to_string())
                .filter(|a| *a != addr.to_string())
                .collect(),
        };

        self.broadcast(&channel_name, msg.into(), addr.into())?;

        return Ok(());
    }

    fn remove_connection_from_all(&mut self, addr: SocketAddr) -> Result<(), String> {
        let mut removed = Vec::new();

        // TODO: Rather than looping through all channels to remove the connection, have each
        // connection store the channels it is a part of. This also make the second part of
        // broadcasting updates much simpler
        for (name, channel) in self.channels.iter_mut() {
            if channel.connections.contains_key(&addr) {
                channel.connections.remove(&addr);

                removed.push(name.clone());

                if channel.connections.len() == 0 {
                    // self.channels.remove(&channel.name);
                }
            }
        }

        for name in removed {
            // TODO: Don't `get` the channels twice if possible
            let channel = self.channels.get(&name).unwrap().clone();

            let msg = Message::Presence {
                channel: name.clone(),
                connections: channel
                    .connections
                    .values()
                    .map(|c| c.addr.to_string())
                    .filter(|a| *a != addr.to_string())
                    .collect(),
            };

            self.broadcast(&channel.name, msg.into(), addr.into())?;
        }

        info!("Removed connection for {}", addr);

        return Ok(());
    }

    fn has_channel(&self, channel_name: String) -> bool {
        self.channels.contains_key(&channel_name)
    }

    fn broadcast(
        &mut self,
        channel_name: &String,
        message: WebSocketMessage,
        skip_addr: Option<SocketAddr>,
    ) -> Result<(), String> {
        if let Some(channel) = self.channels.get(channel_name) {
            let connections = &channel.connections;

            for (addr, sender) in connections.iter() {
                match skip_addr {
                    Some(skip_addr) => {
                        if *addr == skip_addr {
                            continue;
                        }
                    }
                    None => {}
                }

                if let Err(e) = sender.send(message.clone()) {
                    return Err(format!("Error sending message to {}: {}", addr, e));
                }
            }

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }
}
