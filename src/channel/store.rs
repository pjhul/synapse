use axum::extract::ws::Message as WebSocketMessage;
use log::warn;
use serde_json::Value;
use tokio::sync::mpsc::Receiver;

use crate::connection::Connection;
use crate::message::Message;

use super::{router::{Command, CommandResult}, map::ChannelMap, storage::ChannelStorage};

#[derive(Debug)]
pub struct ChannelStore {
    receiver: Receiver<Command>,
    // TODO: When we switch back to a mutex, should probably restructure this into a sharded Mutex, and potentially use an RwLock
    // instead as well
    // See: https://docs.rs/dashmap/latest/dashmap/
    pub channels: ChannelMap,
    db: ChannelStorage,
}

impl ChannelStore {
    pub fn new(rx: Receiver<Command>) -> Self {
        let db = ChannelStorage::new("channels");

        Self {
            receiver: rx,
            channels: ChannelMap::new(),
            db,
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            while let Some(cmd) = self.receiver.recv().await {
                let Command { msg, conn, result } = cmd;

                let cmd_result: CommandResult = match msg.clone() {
                    Message::Join {
                        ref channel,
                        presence,
                    } => {
                        let conn = conn.unwrap();
                        self.handle_join(channel, conn.clone()).into()
                    }
                    Message::Leave { ref channel } => {
                        let conn = conn.unwrap();
                        self.handle_leave(channel, conn.clone()).into()
                    }
                    Message::Disconnect => {
                        let conn = conn.unwrap();
                        self.handle_disconnect(conn.clone()).into()
                    }
                    Message::Broadcast { ref channel, body } => {
                        let conn = conn.unwrap();
                        self.handle_broadcast(channel, body, conn.clone()).into()
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                        Ok(super::router::CommandResponse::Ok)
                    }
                    Message::ChannelGetAll => {
                        self.handle_channel_get_all()
                    }
                    Message::ChannelGet { name } => {
                        self.handle_channel_get(name)
                    }
                    Message::ChannelCreate { name } => {
                        self.handle_channel_create(name)
                    }
                    Message::ChannelDelete { name } => {
                        self.handle_channel_delete(name)
                    }
                    _ => {
                        Err(format!("Received an invalid message: {:?}", msg)).into()
                    }
                };

                if let Some(result) = result {
                    result.send(cmd_result).unwrap();
                }
            }
        });
    }

    // Message handlers

    fn handle_join(&mut self, channel: &String, conn: Connection) -> CommandResult {
        // TODO: Remove this, eventually all channels will have to be created first
        self.channels.add_channel(channel);

        self.channels.add_connection(channel, conn)?;

        let channel = self.channels.get_channel(channel).unwrap().clone();

        self.channels.broadcast(
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

    fn handle_leave(&mut self, channel: &String, conn: Connection) -> CommandResult {
        self.channels.remove_connection(channel, conn.addr)
    }

    fn handle_disconnect(&mut self, conn: Connection) -> CommandResult {
        self.channels.remove_connection_from_all(conn.addr)
    }

    fn handle_broadcast(&mut self, channel: &String, body: Value, conn: Connection) -> CommandResult {
        let msg = Message::Broadcast {
            channel: channel.clone(),
            body,
        };

        self.channels.broadcast(channel, msg.into(), Some(conn.addr))
    }

    // Channel API handlers

    fn handle_channel_get_all(&self) -> CommandResult {
        Ok(super::router::CommandResponse::ChannelGetAll(
            self.channels.keys()
        ))
    }

    fn handle_channel_get(&self, name: String) -> CommandResult {
        if let Some(channel) = self.channels.get_channel(&name) {
            Ok(super::router::CommandResponse::ChannelGet(Some(channel.name.clone())))
        } else {
            Ok(super::router::CommandResponse::ChannelGet(None))
        }
    }

    fn handle_channel_create(&mut self, name: String) -> CommandResult {
        self.channels.add_channel(&name);

        Ok(super::router::CommandResponse::ChannelCreate(name))
    }

    fn handle_channel_delete(&mut self, name: String) -> CommandResult {
        self.channels.remove_channel(&name);

        Ok(super::router::CommandResponse::ChannelDelete(name))
    }
}
