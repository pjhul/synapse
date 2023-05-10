use std::env;

use log::info;
use synapse::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info");
    env_logger::init_from_env(env);

    // let subscriber = tracing_subscriber::fmt()
    //     .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    //     .finish();

    // tracing::subscriber::set_global_default(subscriber)?;

    // console_subscriber::init();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let server = Server::new();
    server.run(&addr).await;

    Ok(())
}
