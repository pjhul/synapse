use std::{collections::HashMap, net::SocketAddr};

use axum::extract::ws::Message as WebSocketMessage;
use log::{error, info, warn};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::connection::Connection;
use crate::message::Message;

#[derive(Debug)]
pub struct Command {
    pub conn: Connection,
    pub msg: Message,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: HashMap<SocketAddr, Connection>,
}

#[derive(Debug)]
pub struct ChannelMap {
    receiver: Receiver<Command>,
    // TODO: When we switch back to a mutex, should probably restructure this into a sharded Mutex, and potentially use an RwLock
    // instead as well
    // See: https://docs.rs/dashmap/latest/dashmap/
    pub channels: HashMap<String, Channel>,
}

#[derive(Clone)]
pub struct ChannelRouter {
    sender: Sender<Command>,
}

impl ChannelRouter {
    pub fn new(tx: Sender<Command>, rx: Receiver<Command>) -> Self {
        let channel_map = ChannelMap::new(rx);
        channel_map.run();

        ChannelRouter { sender: tx }
    }

    pub async fn send_command(&self, msg: Message, conn: Connection) {
        let cmd = Command { conn, msg };

        // TODO: Listen for result here

        self.sender.send(cmd).await.unwrap();
    }
}

impl ChannelMap {
    pub fn new(rx: Receiver<Command>) -> Self {
        ChannelMap {
            receiver: rx,
            channels: HashMap::new(),
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            while let Some(cmd) = self.receiver.recv().await {
                let Command { msg, conn } = cmd;

                // FIXME: This is messy, we should branch and encapsulate this logic better
                match msg {
                    Message::Join {
                        ref channel,
                        presence,
                    } => {
                        self.add_channel(channel);

                        if let Err(e) = self.add_connection(channel, conn.clone()) {
                            // TODO: Package this up into a helper function
                            error!("Failed to add connection: {}", e);

                            conn.send(
                                Message::Error {
                                    message: format!("Failed to add connection: {}", e),
                                }
                                .into(),
                            )
                            .unwrap();
                        } else {
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
                                None
                            )
                            .unwrap();
                        }
                    }
                    Message::Leave { ref channel } => {
                        if let Err(e) = self.remove_connection(channel, conn.addr) {
                            // TODO: Package this up into a helper function
                            error!("Failed to remove connection: {}", e);

                            conn.send(
                                Message::Error {
                                    message: format!("Failed to remove connection: {}", e),
                                }
                                .into(),
                            )
                            .unwrap();
                        }
                    }
                    Message::Disconnect => {
                        if let Err(e) = self.remove_connection_from_all(conn.addr) {
                            error!("Failed to remove connection from all channels: {}", e);

                            conn.send(
                                Message::Error {
                                    message: format!(
                                        "Failed to remove connection from all channels: {}",
                                        e
                                    ),
                                }
                                .into(),
                            )
                            .unwrap();
                        }
                    }
                    Message::Broadcast { ref channel, body } => {
                        let msg = Message::Broadcast {
                            channel: channel.clone(),
                            body,
                        };

                        if let Err(e) =
                            self.broadcast(channel, WebSocketMessage::Text(msg.into()), conn.addr.into())
                        {
                            // TODO: Package this up into a helper function
                            error!("Failed to broadcast message: {}", e);

                            conn.send(
                                Message::Error {
                                    message: format!("Failed to broadcast message: {}", e),
                                }
                                .into(),
                            )
                            .unwrap();
                        }
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                    }
                    _ => {
                        error!("Received an invalid message: {:?}", msg);

                        conn.send(
                            Message::Error {
                                message: format!("Received an invalid message: {:?}", msg),
                            }
                            .into(),
                        )
                        .unwrap();
                    }
                }
            }
        });
    }

    pub fn get_channel(&self, name: &String) -> Option<&Channel> {
        self.channels.get(name)
    }

    pub fn add_channel(&mut self, name: &String) {
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

    pub fn add_connection(
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

    pub fn remove_connection(
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
            connections: channel.unwrap().connections
                .values()
                .map(|c| c.addr.to_string())
                .filter(|a| *a != addr.to_string())
                .collect(),
        };

        self.broadcast(&channel_name, msg.into(), addr.into())?;

            return Ok(());
    }

    pub fn remove_connection_from_all(&mut self, addr: SocketAddr) -> Result<(), String> {
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

    pub fn has_channel(&self, channel_name: String) -> bool {
        self.channels.contains_key(&channel_name)
    }

    pub fn broadcast(
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

                sender.send(message.clone()).unwrap();
            }

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }
}
