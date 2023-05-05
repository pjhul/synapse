use tokio::join;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::connection::Connection;
use crate::message::Message;

use super::store::ChannelStore;

#[derive(Debug)]
pub struct Command {
    pub conn: Connection,
    pub msg: Message,
    pub result: Option<OneshotSender<Result<(), String>>>,
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

    pub async fn send_command(&self, msg: Message, conn: Connection) -> Result<(), String> {
        let (result, mut rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
        let cmd = Command {
            conn,
            msg,
            result: Some(result),
        };

        self.sender.send(cmd).await.unwrap();

        Ok(())

        // let (_, cmd_result) = join!(self.sender.send(cmd), async {
        //     let result = rx.try_recv().unwrap();
        //     result
        // });

        // cmd_result
    }
}
