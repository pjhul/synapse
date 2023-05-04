use futures_channel::oneshot;
use futures_util::{StreamExt, SinkExt};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, tungstenite::protocol::WebSocketConfig, connect_async_with_config};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use std::sync::{Arc, Mutex};

async fn run_concurrent_connections_test(url: &str, max_connections: usize, step: usize) -> Result<(), Box<dyn std::error::Error>> {
    let done_flag = Arc::new(Mutex::new(false));
    let connection_failed_flag = Arc::new(AtomicBool::new(false));

    for connection_count in (0..max_connections).step_by(step) {
        // println!("Testing with {} concurrent connections", connection_count);

        let websocket_config = Some(WebSocketConfig {
            max_send_queue: None,
            max_message_size: None,
            max_frame_size: None,
            accept_unmasked_frames: false,
        });

        for _ in 0..step {
            let url = url.to_string();
            let done_flag_clone = Arc::clone(&done_flag);
            let connection_failed_flag_clone = Arc::clone(&connection_failed_flag);

            tokio::spawn(async move {
                match connect_async_with_config(&url, websocket_config).await {
                    Ok((mut ws_stream, _)) => {
                        while !*done_flag_clone.lock().unwrap() {
                            // Optional: Sleep for a short duration to reduce CPU usage
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }

                        // Close the WebSocket connection
                        ws_stream.close(None).await.unwrap();
                    }
                    Err(err) => {
                        println!("Connection failed: {}", err);
                        // Set the connection_failed_flag to true on connection failure
                        connection_failed_flag_clone.store(true, Ordering::Relaxed);
                    }
                }
            });
        }

        // println!("Successfully connected {} concurrent connections", connection_count);

        // Check if any connection failed
        if connection_failed_flag.load(Ordering::Relaxed) {
            // If the flag is set to true, a connection failed
            panic!("A connection attempt failed");
        }

        // Optional: add a delay between tests
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Signal the tasks to close their connections
    *done_flag.lock().unwrap() = true;

    println!("All connections closed.");

    Ok(())
}

async fn run_message_throughput_test(url: &str, total_connections: usize, messages_per_sec: usize) -> Result<(), Box<dyn std::error::Error>> {
    let done_flag = Arc::new(Mutex::new(false));
    let connection_failed_flag = Arc::new(AtomicBool::new(false));

    for _ in 0..total_connections {
        // println!("Testing with {} concurrent connections", connection_count);

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

                    write.send(Message::Text("{ \"type\": \"join\", \"channel\": \"throughput\" }".to_string())).await;

                    while !*done_flag_clone.lock().unwrap() {
                        // Optional: Sleep for a short duration to reduce CPU usage
                        tokio::time::sleep(Duration::from_millis(100)).await;

                        write.send(Message::Text("{ \"type\": \"broadcast\", \"channel\": \"throughput\", \"body\": \"test\" }".to_string())).await;
                    }

                    ws_stream = write.reunite(read).unwrap();

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
    }

    tokio::time::sleep(Duration::from_secs(60)).await;

    // Signal the tasks to close their connections
    *done_flag.lock().unwrap() = true;

    println!("All connections closed.");

    Ok(())
}


fn main() {
    let url = "ws://main_program:8080/ws";
    let max_connections = 100000;
    let step = 500;

    // Create a multi-threaded runtime for executing the tasks
    let rt = Runtime::new().unwrap();

    // Run the concurrent connections test
    // rt.block_on(run_concurrent_connections_test(url, max_connections, step)).unwrap();

    rt.block_on(run_message_throughput_test(url, 100, 100)).unwrap();
}
