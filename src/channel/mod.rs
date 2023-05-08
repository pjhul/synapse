use std::{collections::HashMap, net::SocketAddr};

use crate::auth::AuthConfig;
use crate::connection::Connection;

mod map;
pub mod router;
mod storage;
mod store;

#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub auth: Option<AuthConfig>,
    pub connections: HashMap<SocketAddr, Connection>,
}
