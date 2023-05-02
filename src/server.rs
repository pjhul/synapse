use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{
        ws::{Message as WebSocketMessage, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};

use futures_util::{future, pin_mut, StreamExt, TryStreamExt};
use log::{error, info, warn};
use tokio::{sync::mpsc, select};

use crate::channel::{ChannelMap, Command};
use crate::message::Message;

pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(self, addr: &str) {
        info!("Listening on: {}", addr);

        let (tx, mut rx) = mpsc::channel::<Command>(1024);
        let _channels = ChannelMap::new(rx);

        let app = Router::new().route(
            "/ws",
            get(
                move |ws: WebSocketUpgrade, conn_info: ConnectInfo<SocketAddr>| {
                    Self::ws_handler(ws, conn_info, tx)
                },
            ),
        );


        select! {
            _ = _channels.run() => {
                error!("ChannelMap exited unexpectedly");
            }
            _ = self.run_server(app, addr) => {
                error!("Server exited unexpectedly");
            }
        }
    }

    async fn run_server(self, app: Router, addr: &str) {
        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        tx: mpsc::Sender<Command>,
    ) -> impl IntoResponse {
        info!("New connection from: {}", addr);
        let tx = tx.clone();

        ws.on_upgrade(move |socket| async move {
            tokio::spawn(async move {
                let addr = addr.to_owned();
                Self::handle_connection(socket, addr, tx).await.unwrap();
            });
        })
    }

    async fn handle_connection(
        stream: WebSocket,
        addr: SocketAddr,
        tx: mpsc::Sender<Command>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = futures_channel::mpsc::unbounded::<WebSocketMessage>();

        let (write, read) = stream.split();

        let broadcast_incoming = read.try_for_each(|msg| {
            let sender = sender.clone();
            let tx = tx.clone();

            async move {
                if let Ok(msg) = msg.to_text() {
                    let msg = match msg.parse::<Message>() {
                        Ok(msg) => msg,
                        Err(e) => {
                            warn!("Received an invalid message: {}", e);

                            let error_msg = Message::Error {
                                message: format!("Invalid message: {}", e),
                            };

                            sender.unbounded_send(WebSocketMessage::Text(error_msg.into())).unwrap();

                            return Ok(());
                        }
                    };

                    let cmd = Command {
                        addr: addr.to_owned(),
                        sender,
                        msg,
                    };

                    if let Err(e) = tx.send(cmd).await {
                        error!("Failed to send command: {}", e);
                    }
                } else {
                    warn!("Received a non-text message");
                }

                Ok(())
            }
        });

        let receive_from_others = receiver.map(Ok).forward(write);

        pin_mut!(broadcast_incoming, receive_from_others);
        future::select(broadcast_incoming, receive_from_others).await;

        tx.send(Command {
            addr: addr.to_owned(),
            sender,
            msg: Message::Disconnect,
        }).await.unwrap();

        Ok(())
    }
}
