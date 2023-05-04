use std::env;

use synapse::server::Server;
use synapse::metrics::get_metrics;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::init_from_env(env);

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let metrics = get_metrics();

            if let Some(num_connections) = metrics.get("Connection::connections") {
                println!("Num connections: {}", num_connections);
            }
        }
    });

    let server = Server::new();
    server.run(&addr).await;

    Ok(())
}
