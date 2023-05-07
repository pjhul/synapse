use std::{collections::HashMap, net::SocketAddr};

use crate::connection::Connection;

pub mod router;
mod store;
mod map;
mod storage;


#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: HashMap<SocketAddr, Connection>,
}
