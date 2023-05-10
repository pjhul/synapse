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
    
    // TODO: Create a testing framework for describing and running scenarios in code
    //
    // Very rough sketch of what this might look like
    // let scenario = Scenario::new()
    //    .with_name("Concurrent Connections Test")
    //    .with_description("Test the server's ability to handle concurrent connections")
    //    .connections(100)
    //    .subscribe_to("test_channel", 100%)
    //    .publish_to("test_channel", 100%) // Percentage of connections that should be publishing
    //    .wait_for(10s)
    //    .run();
    //
}
