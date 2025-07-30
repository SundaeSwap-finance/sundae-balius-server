use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use balius_runtime::{Store, kv::memory::MemoryKv};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    config::AppConfig,
    error::{ApiError, ApiResult},
    keys::{KeyService, PersistentSignerProvider},
};

pub struct WorkerService {
    next_id: usize,
    config: AppConfig,
    keys: KeyService,
    client: reqwest::Client,
    predefined_workers: HashMap<String, Vec<u8>>,
}

impl WorkerService {
    pub fn new(
        config: AppConfig,
        keys: KeyService,
        predefined_workers: HashMap<String, Vec<u8>>,
    ) -> Result<Self> {
        Ok(Self {
            next_id: 1,
            config,
            keys,
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(10))
                .build()?,
            predefined_workers,
        })
    }

    pub async fn create_worker(&mut self, spec: &str) -> ApiResult<Worker> {
        let parsed: WorkerSpec = serde_json::from_str(spec)?;

        let id = self.next_id;
        let worker = self
            .build_balius_worker(id, &parsed.url, parsed.config)
            .await?;
        self.next_id += 1;
        Ok(worker)
    }

    async fn build_balius_worker(
        &mut self,
        id: usize,
        url: &Url,
        config: serde_json::Value,
    ) -> ApiResult<Worker> {
        let store_dir_path = self.config.data_dir.join("stores");
        tokio::fs::create_dir_all(&store_dir_path).await?;
        let store_path = store_dir_path.join(format!("{id}.redb"));
        let store = Store::open(store_path, None)?;

        let ledger =
            balius_runtime::ledgers::u5c::Ledger::new(&balius_runtime::ledgers::u5c::Config {
                endpoint_url: self.config.utxorpc.endpoint_url.clone(),
                headers: self.config.utxorpc.headers.clone(),
            })
            .await?;

        let kv = Arc::new(RwLock::new(MemoryKv::default()));
        let signer = PersistentSignerProvider::new(self.keys.clone());

        let worker = balius_runtime::RuntimeBuilder::new(store)
            .with_ledger(balius_runtime::ledgers::Ledger::U5C(ledger))
            .with_kv(balius_runtime::kv::Kv::Memory(kv))
            .with_logger(balius_runtime::logging::Logger::Tracing)
            .with_signer(balius_runtime::sign::Signer::Custom(Arc::new(Mutex::new(
                signer,
            ))))
            .with_http(balius_runtime::http::Http::Reqwest(self.client.clone()))
            .build()?;

        if url.scheme() == "file" {
            let Some(wasm) = self.predefined_workers.get(url.path()) else {
                return Err(ApiError::new(
                    StatusCode::NOT_FOUND,
                    format!("Worker {url} not found"),
                ));
            };
            worker
                .register_worker(&id.to_string(), wasm, config)
                .await?;
        } else {
            worker
                .register_worker_from_url(&id.to_string(), url, config)
                .await?;
        }

        let token = CancellationToken::new();

        let runtime = worker.clone();
        let cancel = token.child_token();
        let u5c_config = self.config.utxorpc.clone();
        tokio::task::spawn(async move {
            use balius_runtime::drivers::chainsync::{Config, run};
            let config = Config {
                endpoint_url: u5c_config.endpoint_url,
                headers: u5c_config.headers,
            };
            match run(config, runtime, cancel).await {
                Ok(_) => println!("Worker done!"),
                Err(e) => println!("Error! {e:#?}"),
            }
        });

        Ok(Worker {
            id: id.to_string(),
            runtime: worker,
            token,
        })
    }
}

pub struct Worker {
    pub id: String,
    runtime: balius_runtime::Runtime,
    token: CancellationToken,
}

impl Worker {
    pub async fn invoke(
        &mut self,
        method: &str,
        params: &serde_json::Value,
    ) -> ApiResult<serde_json::Value> {
        use balius_runtime::Response;
        let params = serde_json::to_vec(params)?;
        let res = self
            .runtime
            .handle_request(&self.id, method, params)
            .await?;
        Ok(match res {
            Response::Acknowledge => json!({}),
            Response::Json(x) => serde_json::from_slice(&x)?,
            Response::Cbor(x) => json!({ "cbor": hex::encode(x) }),
            Response::PartialTx(x) => json!({ "tx": hex::encode(x) }),
        })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct WorkerSpec {
    network: String,
    operator_version: String,
    throughput_tier: String,

    display_name: String,
    url: Url,
    config: serde_json::Value,
    version: String,
}
