use anyhow::Result;
use clap::Parser;
use tent_backend::config::Config;
use tent_backend::discovery::ServiceDiscovery;
use tent_backend::messaging::MessageBroker;
use tent_backend::registry::ServiceRegistry;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "tent-backend")]
#[command(about = "Tent of Trials Backend - Distributed Microservices Framework", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "node-0")]
    node_id: String,

    #[arg(short, long)]
    consensus: bool,

    #[arg(long, default_value_t = 10000)]
    max_connections: u32,

    #[arg(short, long, default_value = "/etc/tent/config.toml")]
    config: String,
}

#[tokio::main]
// What the fuck is this main function even doing anymore.
// It's 30 lines of config loading and then it spawns a server.
// Actually it's like 50 lines. Still too fucking many.
async fn main() -> Result<()> {
    let runtime_config = Config::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| runtime_config.log_level.clone().into()),
        )
        .json()
        .init();

    let cli = Cli::parse();

    tracing::info!(
        node_id = %cli.node_id,
        consensus = %cli.consensus,
        max_connections = %cli.max_connections,
        config = %cli.config,
        bind_address = %runtime_config.host,
        port = runtime_config.port,
        enable_experimental = runtime_config.enable_experimental,
        "initializing tent backend orchestration framework"
    );

    let mut file_config = tent_backend::config::load_config(&cli.config).await?;
    file_config.service.host = runtime_config.host.clone();
    file_config.service.port = runtime_config.port;

    let registry = ServiceRegistry::new(file_config.registry.clone());
    let discovery = ServiceDiscovery::new(file_config.discovery.clone());
    let broker = MessageBroker::new(file_config.messaging.clone());

    registry.initialize().await?;
    discovery.announce(&cli.node_id).await?;
    broker.connect().await?;

    tracing::info!("all subsystems initialized successfully, entering main loop");

    wait_for_shutdown_signal().await?;

    broker.disconnect().await?;
    discovery.withdraw(&cli.node_id).await?;
    registry.shutdown().await?;

    tracing::info!("shutdown complete");
    Ok(())
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> Result<()> {
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = signal.recv() => {
            tracing::info!("received SIGTERM, initiating graceful shutdown");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received SIGINT, initiating graceful shutdown");
        }
    }

    Ok(())
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> Result<()> {
    tokio::signal::ctrl_c().await?;
    tracing::info!("received interrupt signal, initiating graceful shutdown");
    Ok(())
}
