
# Elock

![License](https://img.shields.io/badge/License--MIT-black?logo=DISCLAIMER)
![Rust](https://img.shields.io/badge/-Rust-black?logo=rust&logoColor=white)
![Kubernetes](https://img.shields.io/badge/-Kubernetes-black?&logo=kubernetes&logoColor=white)
![etcd](https://img.shields.io/badge/-etcd-black?&logo=etcd&logoColor=white)


> A lightweight distributed lock implementation built on top of etcd using leases and watch

### Install

```bash
cargo add elock
```

### Example
You can new a `LockManager` to manage etcd locks.

```rust

use elock::LockManager;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut manager = LockManager::new("127.0.0.1:2379")
        .with_tls("/etc/etcd/ssl/etcd-ca.pem", "/etc/etcd/ssl/etcd-client.pem", "/etc/etcd/ssl/etcd-client-key.pem")
        .init()
        .await;
    let mut locker = manager.create_lock("/test/lock").await;
    
    // Duration sets the timeout for acquiring a lock, similar to context.WithTimeout in Golang.
    locker.lock(Duration::from_secs(30)).await.unwrap();

    // Do something

    locker.unlock().await.unwrap();
    
    tracing::info!("Unlock successful");
}

```