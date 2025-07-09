use etcd_client::{Certificate, Client, ConnectOptions, Identity, LockOptions, TlsOptions};
use std::fs;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tokio_util::sync::CancellationToken;

const DEFAULT_TTL: i64 = 10;

pub struct LockManager {
    client: Option<Client>,
    endpoint: String,
    options: Option<ConnectOptions>,
}

impl LockManager {
    pub fn new(endpoint: &str) -> Self {
        Self {
            client: None,
            options: None,
            endpoint: endpoint.to_string(),
        }
    }

    pub fn with_tls(
        mut self,
        ca_path: &str,
        cert_client_path: &str,
        client_key_path: &str,
    ) -> Self {
        let config = TlsOptions::new()
            .ca_certificate(Certificate::from_pem(fs::read_to_string(ca_path).unwrap()))
            .identity(Identity::from_pem(
                fs::read_to_string(cert_client_path).unwrap(),
                fs::read_to_string(client_key_path).unwrap(),
            ));

        let options = ConnectOptions::default().with_tls(config);

        self.options = Some(options);
        self
    }
    
    pub fn with_custom_options(mut self, option: ConnectOptions) -> Self {
        self.options = Some(option);
        self
    }

    pub async fn init(mut self) -> Self {
        let client = Client::connect([self.endpoint.clone()], self.options.clone())
            .await
            .unwrap();

        self.client = Some(client);
        self
    }

    pub async fn create_lock(&mut self, key: &str) -> Lock {
        let client = self.client.clone().unwrap();
        Lock::new(client.clone(), key.to_string())
    }
}

#[derive(Clone)]
pub struct Lock {
    client: Client,
    key: String,
    current_key: Vec<u8>,
    cancel_token: Option<CancellationToken>,
}

impl Lock {
    fn new(client: Client, key: String) -> Self {
        Self {
            client,
            key,
            current_key: Vec::new(),
            cancel_token: None,
        }
    }
    pub async fn lock(
        &mut self,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tokio::select! {
            _ = sleep(timeout) => {
                Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut,"timeout")))
            }

            res = self.try_acquire(DEFAULT_TTL) => {
                tracing::info!("lock {} success", self.key);
                res
            }
        }
    }

    async fn try_acquire(&mut self, ttl: i64) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();

        let mut ticker = interval(Duration::from_secs(2));

        loop {
            tracing::debug!("[{}] try to lock", self.key);

            let lease = client.lease_grant(ttl, None).await?;
            let lease_id = lease.id();

            let options = LockOptions::new().with_lease(lease_id);

            match client.lock(self.key.as_bytes(), Some(options)).await {
                Ok(resp) => {
                    let token = CancellationToken::new();
                    let cancel = token.clone();

                    self.current_key = Vec::from(resp.key());
                    self.cancel_token = Some(token);
                    self.keep_alive(lease_id, ttl, cancel).await?;
                    return Ok(());
                }
                Err(err) => {
                    tracing::debug!("error locking lease {:?}", err);
                }
            };

            ticker.tick().await;
        }
    }

    async fn keep_alive(
        &self,
        lease_id: i64,
        ttl: i64,
        cancel: CancellationToken,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        tokio::spawn(async move {
            let (mut keeper, mut stream) = match client.lease_keep_alive(lease_id).await {
                Ok(stream) => stream,
                Err(_) => {
                    return;
                }
            };

            let mut ticker = interval(Duration::from_secs((ttl / 2) as u64));

            loop {
                ticker.tick().await;

                keeper.keep_alive().await.unwrap();

                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::debug!("[{}] kill lease", lease_id);
                        return;
                    }

                    resp = stream.message() => {

                        match resp {
                            Ok(Some(resp)) => {
                                tracing::debug!("response: lease_id={}, ttl={}", lease_id, resp.ttl());
                            }
                            Ok(None) => {
                                tracing::debug!("done: lease_id={}", lease_id);
                                break;
                            }
                            Err(e) => {
                                tracing::debug!("error: lease_id={}, error={}", lease_id,e);
                                break;
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn unlock(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = self.client.clone();
        if let Some(toke) = &self.cancel_token {
            toke.cancel();
            self.cancel_token = None;
            
            let key = self.current_key.clone();
            if key.is_empty() {
                return Ok(());
            }
            client.unlock(key).await?;
            
            if let Ok(str) = String::from_utf8(self.current_key.clone()) {
                tracing::info!("unlock {} success", str);
            };
        }
        Ok(())
    }
}
