use std::str::FromStr;

use serde::{Deserialize, Serialize};

use axum::extract::ws::Message as WebSocketMessage;

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

impl Into<String> for Message {
    fn into(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

impl Into<WebSocketMessage> for Message {
    fn into(self) -> WebSocketMessage {
        WebSocketMessage::Text(self.into())
    }
}
