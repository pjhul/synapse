use std::{collections::HashMap, net::SocketAddr};

use crate::connection::Connection;

pub mod router;
mod store;
mod map;


#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub connections: HashMap<SocketAddr, Connection>,
}
