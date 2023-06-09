use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::connection::Connection;
use crate::message::Message;
use crate::metrics::Metrics;

use super::store::ChannelStore;

#[derive(Debug)]
pub struct Command {
    pub msg: Message,
    pub conn: Option<Connection>,
    pub result: Option<OneshotSender<CommandResult>>,
}

pub type CommandResult = Result<CommandResponse, String>;

#[derive(Debug)]
pub enum CommandResponse {
    Ok,
    Unauthorized(String),
    ChannelGetAll(Vec<String>),
    ChannelCreate(String),
    ChannelGet(Option<String>),
    ChannelDelete(String),
}

#[derive(Clone)]
pub struct ChannelRouter {
    sender: Sender<Command>,
}

/// Public interface for interracting with Channels. Effectively just a wrapper around the
/// MPSC Sender for Commands to the inner ChannelStore
impl ChannelRouter {
    pub fn new(tx: Sender<Command>, rx: Receiver<Command>) -> Self {
        let channel_map = ChannelStore::new(rx);
        channel_map.run();

        ChannelRouter { sender: tx }
    }

    pub async fn send_command(&self, msg: Message, conn: Option<Connection>) -> CommandResult {
        let (result, rx) = tokio::sync::oneshot::channel::<CommandResult>();
        let cmd = Command {
            conn,
            msg,
            result: Some(result),
        };

        ChannelRouter::increment_command_buffer_size();

        self.sender.send(cmd).await.unwrap();

        rx.await.unwrap()
    }
}

impl Metrics for ChannelRouter {}
