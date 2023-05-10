use messages::run_message_throughput_test;
use tokio::runtime::Runtime;

mod messages;
mod connections;

fn main() {
    let url = "ws://172.17.0.1:8080/ws";
    let max_connections = 1000;
    let step = 500;

    // Create a multi-threaded runtime for executing the tasks
    let rt = Runtime::new().unwrap();

    // Run the concurrent connections test
    // rt.block_on(run_concurrent_connections_test(url, max_connections, step)).unwrap();

    rt.block_on(run_message_throughput_test(url, max_connections, 100))
        .unwrap();
}
