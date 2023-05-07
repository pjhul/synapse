use std::{collections::HashMap, net::SocketAddr};

use axum::extract::ws::Message as WebSocketMessage;
use log::info;

use crate::connection::Connection;
use crate::message::Message;

use super::{router::CommandResult, storage::ChannelStorage, Channel};

#[derive(Debug)]
pub struct ChannelMap {
    pub channels: HashMap<String, Channel>,
    db: ChannelStorage,
}

impl ChannelMap {
    pub fn new() -> Self {
        let db = ChannelStorage::new("db");

        let channels = db.get_channels().unwrap_or_else(|e| {
            panic!("Error loading channels from DB: {}", e);
        });

        let channel_map = channels
            .into_iter()
            .map(|c| {
                (
                    c.clone(),
                    Channel {
                        name: c.clone(),
                        connections: HashMap::new(),
                    },
                )
            })
            .collect();

        Self {
            channels: channel_map,
            db,
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    pub fn get_channel(&self, name: &String) -> Option<&Channel> {
        self.channels.get(name)
    }

    pub fn has_channel(&self, channel_name: String) -> bool {
        self.channels.contains_key(&channel_name)
    }

    pub fn add_channel(&mut self, name: &String) -> Result<(), String> {
        // We update the DB first here as that can fail but the write to the hashmap cannot, and so
        // no rollback is needed. I think we'll still need more robust logic here to keep these two
        // in-sync
        self.db.create_channel(name)?;

        let channels = &mut self.channels;

        if channels.contains_key(name) {
            // TODO: Unclear if we should fail here or not, but I'm leaning towards not failing
            // return Err(format!("Channel {} already exists", name));
        } else {
            channels.insert(
                name.clone(),
                Channel {
                    name: name.clone(),
                    connections: HashMap::new(),
                },
            );
        }

        Ok(())
    }

    pub fn remove_channel(&mut self, name: &String) -> Result<(), String> {
        self.db.remove_channel(name)?;

        let channels = &mut self.channels;

        if channels.contains_key(name) {
            channels.remove(name);
        }

        Ok(())
    }

    pub fn add_connection(&mut self, channel_name: &String, conn: Connection) -> CommandResult {
        if let Some(channel) = self.channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            if connections.contains_key(&conn.addr) {
                info!("Connection already exists for {}", conn.addr);
                return Err(format!("Connection already exists for {}", conn.addr));
            }

            info!("Added connection for {}", conn.addr);
            connections.insert(conn.addr, conn);

            return Ok(super::router::CommandResponse::Ok);
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }

    pub fn remove_connection(&mut self, channel_name: &String, addr: SocketAddr) -> CommandResult {
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

        return Ok(super::router::CommandResponse::Ok);
    }

    pub fn remove_connection_from_all(&mut self, addr: SocketAddr) -> CommandResult {
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

        return Ok(super::router::CommandResponse::Ok);
    }

    pub fn broadcast(
        &mut self,
        channel_name: &String,
        message: WebSocketMessage,
        skip_addr: Option<SocketAddr>,
    ) -> CommandResult {
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

            return Ok(super::router::CommandResponse::Ok);
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }
    }
}
