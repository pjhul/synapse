use std::{collections::HashMap, net::SocketAddr};

use serde::{Deserialize, Serialize};

use crate::auth::AuthConfig;
use crate::connection::Connection;

mod map;
pub mod router;
mod storage;
mod store;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Channel {
    pub name: String,
    pub auth: Option<AuthConfig>,
    pub presence: bool,
    #[serde(skip)]
    pub connections: HashMap<SocketAddr, Connection>,
}

impl Channel {
    pub fn new(name: String, auth: Option<AuthConfig>) -> Self {
        Self {
            name,
            presence: true,
            auth,
            connections: HashMap::new(),
        }
    }

    // FIXME: This is a hack to get tests to pass, eventually allow the client to set this directly
    pub fn disable_presence(&mut self) {
        self.presence = false;
    }
}

impl AsRef<[u8]> for Channel {
    fn as_ref(&self) -> &[u8] {
        let data = bincode::serialize(self).expect("Failed to serialize channel");
        Box::leak(data.into_boxed_slice())
    }
}

impl From<Box<[u8]>> for Channel {
    fn from(bytes: Box<[u8]>) -> Self {
        bincode::deserialize(&bytes).unwrap()
    }
}
