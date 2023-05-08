use axum::extract::ws::Message as WebSocketMessage;
use log::warn;
use serde_json::Value;
use tokio::sync::mpsc::Receiver;

use crate::auth::AuthPayload;
use crate::message::Message;
use crate::{auth::make_auth_request, connection::Connection};

use super::router::CommandResponse;
use super::{
    map::ChannelMap,
    router::{Command, CommandResult},
    storage::{ChannelStorage, Storage},
};

#[derive(Debug)]
pub struct ChannelStore {
    receiver: Receiver<Command>,
    // TODO: When we switch back to a mutex, should probably restructure this into a sharded Mutex, and potentially use an RwLock
    // instead as well
    // See: https://docs.rs/dashmap/latest/dashmap/
    pub channels: ChannelMap<ChannelStorage>,
}

impl ChannelStore {
    pub fn new(rx: Receiver<Command>) -> Self {
        let storage = ChannelStorage::new("db");

        Self {
            receiver: rx,
            channels: ChannelMap::new(storage),
        }
    }

    pub fn run(mut self) {
        tokio::spawn(async move {
            while let Some(cmd) = self.receiver.recv().await {
                let Command { msg, conn, result } = cmd;

                let cmd_result: CommandResult = match msg.clone() {
                    Message::Join {
                        ref channel,
                        presence: _,
                    } => {
                        let conn = conn.unwrap();
                        self.handle_join(channel, conn.clone()).await
                    }
                    Message::Leave { ref channel } => {
                        let conn = conn.unwrap();
                        self.handle_leave(channel, conn.clone())
                    }
                    Message::Disconnect => {
                        let conn = conn.unwrap();
                        self.handle_disconnect(conn.clone())
                    }
                    Message::Broadcast { ref channel, body } => {
                        let conn = conn.unwrap();
                        self.handle_broadcast(channel, body, conn.clone())
                    }
                    Message::Error { message } => {
                        warn!("Received an error message: {}", message);
                        Ok(super::router::CommandResponse::Ok)
                    }
                    Message::ChannelGetAll => self.handle_channel_get_all(),
                    Message::ChannelGet { name } => self.handle_channel_get(name),
                    Message::ChannelCreate { name } => self.handle_channel_create(name),
                    Message::ChannelDelete { name } => self.handle_channel_delete(name),
                    _ => Err(format!("Received an invalid message: {:?}", msg)),
                };

                if let Some(result) = result {
                    result.send(cmd_result).unwrap();
                }
            }
        });
    }

    // Message handlers

    async fn handle_join(&mut self, channel_name: &String, conn: Connection) -> CommandResult {
        let channel = self.channels.get_channel(channel_name);

        if channel.is_none() {
            return Err(format!("Channel {} does not exist", channel_name));
        }

        let channel = channel.unwrap().clone();

        if let Some(ref auth) = channel.auth {
            let is_authorized = make_auth_request(
                auth,
                AuthPayload {
                    channel: channel.name.clone(),
                    conn_id: conn.id.clone(),
                },
            )
            .await?;

            if !is_authorized {
                return Ok(CommandResponse::Unauthorized("Unauthorized".to_string()));
            }
        }

        self.channels.add_connection(channel_name, conn)?;

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

    fn handle_broadcast(
        &mut self,
        channel: &String,
        body: Value,
        conn: Connection,
    ) -> CommandResult {
        let msg = Message::Broadcast {
            channel: channel.clone(),
            body,
        };

        self.channels
            .broadcast(channel, msg.into(), Some(conn.addr))
    }

    // Channel API handlers

    fn handle_channel_get_all(&self) -> CommandResult {
        Ok(CommandResponse::ChannelGetAll(self.channels.keys()))
    }

    fn handle_channel_get(&self, name: String) -> CommandResult {
        if let Some(channel) = self.channels.get_channel(&name) {
            Ok(CommandResponse::ChannelGet(Some(channel.name.clone())))
        } else {
            Ok(CommandResponse::ChannelGet(None))
        }
    }

    fn handle_channel_create(&mut self, name: String) -> CommandResult {
        self.channels.add_channel(&name)?;

        Ok(CommandResponse::ChannelCreate(name))
    }

    fn handle_channel_delete(&mut self, name: String) -> CommandResult {
        self.channels.remove_channel(&name)?;

        Ok(CommandResponse::ChannelDelete(name))
    }
}
