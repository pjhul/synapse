use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{
    connect_async_with_config, tungstenite::protocol::WebSocketConfig,
};

use std::sync::{Arc, Mutex};

pub async fn run_message_throughput_test(
    url: &str,
    total_connections: usize,
    messages_per_sec: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let done_flag = Arc::new(Mutex::new(false));
    let connection_failed_flag = Arc::new(AtomicBool::new(false));

    println!("Testing with {} concurrent connections", total_connections);

    for _ in 0..total_connections {
        let websocket_config = Some(WebSocketConfig {
            max_send_queue: None,
            max_message_size: None,
            max_frame_size: None,
            accept_unmasked_frames: false,
        });

        let url = url.to_string();
        let done_flag_clone = Arc::clone(&done_flag);
        let connection_failed_flag_clone = Arc::clone(&connection_failed_flag);

        tokio::spawn(async move {
            match connect_async_with_config(&url, websocket_config).await {
                Ok((mut ws_stream, _)) => {
                    let (mut write, read) = ws_stream.split();

                    write
                        .send(Message::Text(
                            "{ \"type\": \"join\", \"channel\": \"throughput\" }".to_string(),
                        ))
                        .await;

                    // !*done_flag_clone.lock().unwrap()
                    while true {
                        // Optional: Sleep for a short duration to reduce CPU usage
                        tokio::time::sleep(Duration::from_millis(100)).await;

                        write.send(Message::Text("{ \"type\": \"broadcast\", \"channel\": \"throughput\", \"body\": \"test\" }".to_string())).await;
                    }

                    ws_stream = write.reunite(read).unwrap();

                    println!("Connection closed.");

                    // Close the WebSocket connection
                    ws_stream.close(None).await.unwrap();
                }
                Err(err) => {
                    println!("Connection failed: {}", err);
                    // Set the connection_failed_flag to true on connection failure
                    // connection_failed_flag_clone.store(true, Ordering::Relaxed);
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tokio::time::sleep(Duration::from_secs(600)).await;

    // Signal the tasks to close their connections
    // *done_flag.lock().unwrap() = true;

    println!("All connections closed.");

    Ok(())
}
