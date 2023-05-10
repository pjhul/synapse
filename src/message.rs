use std::str::FromStr;

use serde::{Deserialize, Serialize};

use axum::extract::ws::Message as WebSocketMessage;

use crate::auth::AuthConfig;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Join {
        channel: String,
        #[serde(default)]
        presence: bool,
    },
    Leave {
        channel: String,
    },
    Disconnect,
    Broadcast {
        channel: String,
        body: serde_json::Value,
    },
    Presence {
        channel: String,
        connections: Vec<String>,
    },
    Error {
        message: String,
    },

    // Channel API
    ChannelGetAll,
    ChannelGet {
        name: String,
    },
    ChannelCreate {
        name: String,
        auth: Option<AuthConfig>,
        presence: bool,
    },
    ChannelDelete {
        name: String,
    },
}

pub enum ChannelApiMessage {
    GetChannels,
    CreateChannel { name: String },
    DeleteChannel { name: String },
    GetChannel { name: String },
}

impl FromStr for Message {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl From<Message> for String {
    fn from(msg: Message) -> Self {
        serde_json::to_string(&msg).unwrap()
    }
}

impl From<Message> for WebSocketMessage {
    fn from(msg: Message) -> Self {
        WebSocketMessage::Text(msg.into())
    }
}
