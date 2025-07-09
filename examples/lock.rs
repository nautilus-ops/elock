use elock::LockManager;
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut manager = LockManager::new("127.0.0.1:2379")
        .with_tls("/etc/etcd/ssl/etcd-ca.pem", "/etc/etcd/ssl/etcd-client.pem", "/etc/etcd/ssl/etcd-client-key.pem")
        .init()
        .await;
    let mut lock = manager.create_lock("/test/lock").await;

    lock.lock(Duration::from_secs(30)).await.unwrap();

    // Do something

    lock.unlock().await.unwrap();
    tracing::info!("Unlock successful");
}
