use std::{collections::HashMap, net::SocketAddr};

use axum::extract::ws::Message as WebSocketMessage;
use futures_channel::mpsc::UnboundedSender;
use log::{error, info, warn};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::message::Message;

type ChannelSender = UnboundedSender<WebSocketMessage>;

#[derive(Debug)]
pub struct Command {
    pub addr: SocketAddr,
    pub sender: ChannelSender,
    pub msg: Message,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: HashMap<SocketAddr, ChannelSender>,
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
        let cmap = ChannelMap::new(rx);
        cmap.run();

        ChannelRouter { sender: tx }
    }

    pub async fn send_command(&self, msg: Message, sender: ChannelSender, addr: SocketAddr) {
        let cmd = Command { addr, sender, msg };

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
                let Command { addr, msg, sender } = cmd;

                // FIXME: This is messy, we should branch and encapsulate this logic better
                match msg {
                    Message::Join { ref channel } => {
                        self.add_channel(channel);

                        if let Err(e) = self.add_connection(channel, addr, sender.clone()) {
                            // TODO: Package this up into a helper function
                            error!("Failed to add connection: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to add connection: {}", e),
                            };

                            sender
                                .unbounded_send(WebSocketMessage::Text(error_msg.into()))
                                .unwrap();
                        }
                    }
                    Message::Leave { ref channel } => {
                        if let Err(e) = self.remove_connection(channel, addr) {
                            // TODO: Package this up into a helper function
                            error!("Failed to remove connection: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to remove connection: {}", e),
                            };

                            sender
                                .unbounded_send(WebSocketMessage::Text(error_msg.into()))
                                .unwrap();
                        }
                    }
                    Message::Disconnect => {
                        if let Err(e) = self.remove_connection_from_all(addr) {
                            error!("Failed to remove connection from all channels: {}", e);

                            let error_msg = Message::Error {
                                message: format!(
                                    "Failed to remove connection from all channels: {}",
                                    e
                                ),
                            };

                            sender
                                .unbounded_send(WebSocketMessage::Text(error_msg.into()))
                                .unwrap();
                        }
                    }
                    Message::Broadcast { ref channel, body } => {
                        let msg = Message::Broadcast {
                            channel: channel.clone(),
                            body,
                        };

                        if let Err(e) =
                            self.broadcast(channel, addr, WebSocketMessage::Text(msg.into()))
                        {
                            // TODO: Package this up into a helper function
                            error!("Failed to broadcast message: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Failed to broadcast message: {}", e),
                            };

                            sender
                                .unbounded_send(WebSocketMessage::Text(error_msg.into()))
                                .unwrap();
                        }
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                    }
                }
            }
        });
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
        addr: SocketAddr,
        sender: ChannelSender,
    ) -> Result<(), String> {
        let channels = &mut self.channels;

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

    pub fn remove_connection(
        &mut self,
        channel_name: &String,
        addr: SocketAddr,
    ) -> Result<(), String> {
        let channels = &mut self.channels;

        if let Some(channel) = channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            connections.remove(&addr);
            info!("Removed connection for {}", addr);

            return Ok(());
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }

    pub fn remove_connection_from_all(&mut self, addr: SocketAddr) -> Result<(), String> {
        let channels = &mut self.channels;

        for (_, channel) in channels.iter_mut() {
            let connections = &mut channel.connections;
            connections.remove(&addr);
        }

        info!("Removed connection for {}", addr);

        return Ok(());
    }

    pub fn has_channel(&self, channel_name: String) -> bool {
        self.channels.contains_key(&channel_name)
    }

    pub fn broadcast(
        &self,
        channel_name: &String,
        skip_addr: SocketAddr,
        message: WebSocketMessage,
    ) -> Result<(), String> {
        if let Some(channel) = self.channels.get(channel_name) {
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
