use std::{collections::HashMap, net::SocketAddr};

use log::info;
use tungstenite::protocol::Message as WebSocketMessage;

use crate::message::Message;
use crate::metrics::Metrics;
use crate::{auth::AuthConfig, connection::Connection};

use super::{router::CommandResult, storage::Storage, Channel};

#[derive(Debug)]
pub struct ChannelMap<S: Storage> {
    pub channels: HashMap<String, Channel>,
    db: S,
}

impl<S: Storage> ChannelMap<S> {
    pub fn new(db: S) -> Self {
        // Eventually we could consider having some code like this to automatically switch between a mock DB and a real DB
        // let db = if cfg!(test) {
        //     MockStorageBackend {}
        // } else {
        //     ChannelStorage::new("db")
        // };

        let channels = db.get_channels().unwrap_or_else(|e| {
            panic!("Error loading channels from DB: {}", e);
        });

        let channel_map = channels.into_iter().map(|c| (c.name.clone(), c)).collect();

        Self {
            channels: channel_map,
            db,
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    pub fn get(&self, name: &String) -> Option<&Channel> {
        self.channels.get(name)
    }

    pub fn get_mut(&mut self, name: &String) -> Option<&mut Channel> {
        self.channels.get_mut(name)
    }

    pub fn has_channel(&self, channel_name: String) -> bool {
        self.channels.contains_key(&channel_name)
    }

    pub fn add_channel(
        &mut self,
        name: &String,
        auth: Option<AuthConfig>,
        presence: bool,
    ) -> Result<(), String> {
        let channel = Channel::new(name.clone(), auth, presence);

        // We update the DB first here as that can fail but the write to the hashmap cannot, and so
        // no rollback is needed. I think we'll still need more robust logic here to keep these two
        // in-sync
        self.db.create_channel(&channel)?;

        let channels = &mut self.channels;

        if channels.contains_key(name) {
            // TODO: Unclear if we should fail here or not, but I'm leaning towards not failing
            // return Err(format!("Channel {} already exists", name));
        } else {
            channels.insert(name.clone(), channel);
        }

        Ok(())
    }

    pub fn remove_channel(&mut self, name: &String) -> Result<(), String> {
        // TODO: Alert all the connections that the connection to the channel has closed
        self.db.remove_channel(name)?;

        let channels = &mut self.channels;

        if channels.contains_key(name) {
            channels.remove(name);
        }

        Ok(())
    }

    pub async fn add_connection(
        &mut self,
        channel_name: &String,
        conn: Connection,
        send_presence: bool,
    ) -> CommandResult {
        if let Some(channel) = self.channels.get_mut(channel_name) {
            let connections = &mut channel.connections;

            if connections.contains_key(&conn.addr) {
                info!("Connection already exists for {}", conn.addr);
                return Err(format!("Connection already exists for {}", conn.addr));
            }

            info!("Added connection for {}", conn.addr);
            connections.insert(conn.addr, conn);

            if send_presence {
                self.broadcast_presence(channel_name).await?;
            }

            Ok(super::router::CommandResponse::Ok)
        } else {
            Err(format!("Channel {} does not exist", channel_name))
        }
    }

    pub async fn remove_connection(
        &mut self,
        channel_name: &String,
        addr: SocketAddr,
    ) -> CommandResult {
        let channel = self.channels.get_mut(channel_name);

        if let Some(channel) = channel {
            channel.connections.remove(&addr);
            info!("Removed connection for {}", addr);

            if channel.presence {
                self.broadcast_presence(channel_name).await?;
            }
        } else {
            return Err(format!("Channel {} does not exist", channel_name));
        }

        Ok(super::router::CommandResponse::Ok)
    }

    pub async fn remove_connection_from_all(&mut self, addr: SocketAddr) -> CommandResult {
        let mut presence_updates = Vec::new();

        // TODO: Rather than looping through all channels to remove the connection, have each
        // connection store the channels it is a part of. This also make the second part of
        // broadcasting updates much simpler
        for (name, channel) in self.channels.iter_mut() {
            if channel.connections.contains_key(&addr) {
                channel.connections.remove(&addr);

                if channel.presence {
                    presence_updates.push(name.clone());
                }

                if channel.connections.is_empty() {
                    // self.channels.remove(&channel.name);
                }
            }
        }

        for name in presence_updates {
            self.broadcast_presence(&name).await?;
        }

        info!("Removed connection for {}", addr);

        Ok(super::router::CommandResponse::Ok)
    }

    pub async fn broadcast(
        &mut self,
        channel_name: &str,
        message: WebSocketMessage,
        skip_addr: Option<SocketAddr>,
    ) -> CommandResult {
        if let Some(channel) = self.channels.get(channel_name) {
            let connections = &channel.connections;

            let broadcast_tasks = connections
                .iter()
                .filter(|(addr, _)| {
                    if let Some(skip_addr) = skip_addr {
                        **addr != skip_addr
                    } else {
                        true
                    }
                })
                .map(|(_, sender)| sender.send(message.clone()))
                .collect::<Vec<_>>();

            info!(
                "Broadcasting message to {} connections",
                broadcast_tasks.len()
            );

            let results = futures::future::join_all(broadcast_tasks).await;

            Ok(super::router::CommandResponse::Ok)
        } else {
            Err(format!("Channel {} does not exist", channel_name))
        }
    }

    // FIXME: When adding channels too quickly, this function seems to hang which causes incoming
    // messages to back up.
    // It probably makes sense to separate the logic for sending presence updates to de-couple it
    // from big spikes in messages and vice-versa.
    pub async fn broadcast_presence(&mut self, channel_name: &str) -> CommandResult {
        let channel = self.channels.get(channel_name).unwrap();

        if !channel.presence {
            return Ok(super::router::CommandResponse::Ok);
        }

        let connections = channel.connections.values();

        let msg = Message::Presence {
            channel: String::from(channel_name),
            connections: connections.map(|c| c.id.clone()).collect(),
        };

        self.broadcast(channel_name, msg.into(), None).await
    }
}

impl<T: Storage> Metrics for ChannelMap<T> {}

// Tests are included only when running tests
#[cfg(test)]
mod tests {
    use futures_util::FutureExt;
    use tokio::sync::mpsc::channel;
    use tokio_stream::{wrappers::ReceiverStream, StreamExt};
    use uuid::Uuid;

    use crate::channel::storage::tests::MockChannelStorage;
    use crate::connection::ConnectionSender;

    use super::*;

    fn create_channel_map() -> ChannelMap<MockChannelStorage> {
        ChannelMap::new(MockChannelStorage::new("/tmp/test.db"))
    }

    fn create_socket_addr() -> SocketAddr {
        "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
    }

    fn create_sender() -> ConnectionSender {
        let (_tx, _rx) = channel(1);
        _tx
    }

    fn create_connection() -> Connection {
        Connection {
            id: Uuid::new_v4().to_string(),
            addr: create_socket_addr(),
            sender: Some(create_sender()),
        }
    }

    #[tokio::test]
    async fn test_add_channel() {
        let mut channel_map = create_channel_map();
        let channel_name = String::from("test_channel");

        channel_map.add_channel(&channel_name, None).unwrap();

        assert!(channel_map.has_channel(channel_name));
    }

    #[tokio::test]
    async fn test_add_connection() {
        let mut channel_map = create_channel_map();
        let channel_name = String::from("test_channel");
        let conn = create_connection();

        channel_map.add_channel(&channel_name, None).unwrap();

        // FIXME: Hack to disable presence
        channel_map
            .get_mut(&channel_name)
            .unwrap()
            .disable_presence();

        channel_map
            .add_connection(&channel_name, conn.clone())
            .await
            .unwrap();

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

        channel_map.add_channel(&channel_name, None).unwrap();

        // FIXME: Hack to disable presence
        channel_map
            .get_mut(&channel_name)
            .unwrap()
            .disable_presence();

        channel_map
            .add_connection(&channel_name, conn.clone())
            .await
            .unwrap();

        channel_map
            .remove_connection(&channel_name, conn.addr)
            .unwrap();

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

        channel_map.add_channel(&channel_name, None).unwrap();

        // FIXME: Hack to disable presence
        channel_map
            .get_mut(&channel_name)
            .unwrap()
            .disable_presence();

        channel_map
            .add_connection(&channel_name, conn.clone())
            .await
            .unwrap();

        channel_map
            .remove_connection_from_all(conn.addr)
            .await
            .unwrap();

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

        channel_map.add_channel(&channel_name, None).unwrap();

        assert!(channel_map.has_channel(channel_name));
    }

    #[tokio::test]
    async fn test_broadcast() {
        let mut channel_map = create_channel_map();
        let channel_name = String::from("test_channel");

        let (sender1, receiver1) = channel(1);
        let (sender2, receiver2) = channel(1);

        let conn1 = Connection {
            id: Uuid::new_v4().to_string(),
            addr: "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
            sender: Some(sender1),
        };

        let conn2 = Connection {
            id: Uuid::new_v4().to_string(),
            addr: "127.0.0.2:8080".parse::<SocketAddr>().unwrap(),
            sender: Some(sender2),
        };

        let skip_addr = conn1.addr;

        channel_map.add_channel(&channel_name, None).unwrap();

        // FIXME: Hack to disable presence
        channel_map
            .get_mut(&channel_name)
            .unwrap()
            .disable_presence();

        channel_map
            .add_connection(&channel_name, conn1)
            .await
            .unwrap();
        channel_map
            .add_connection(&channel_name, conn2)
            .await
            .unwrap();

        let msg_text = "Hello, world!";
        let message = WebSocketMessage::Text(msg_text.to_owned());
        channel_map
            .broadcast(&channel_name, message.clone(), Some(skip_addr))
            .await
            .unwrap();

        let mut receiver1 = ReceiverStream::new(receiver1);
        let mut receiver2 = ReceiverStream::new(receiver2);

        assert!(receiver1.next().now_or_never().is_none());
        let received_msg = receiver2.next().await.unwrap();
        assert_eq!(received_msg, message)
    }
}
