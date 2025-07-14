use serde::Deserialize;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Preview,
    Mainnet,
}

impl Network {
    pub fn to_unix_time(&self, slot: u64) -> u64 {
        let offset = match self {
            Self::Preview => 1666656000,
            Self::Mainnet => 1591566291,
        };
        (slot + offset) * 1000
    }

    pub fn relay_url(&self) -> Url {
        let url = match self {
            Self::Preview => "http://sse-relay.preview.sundae.fi/publish",
            Self::Mainnet => "http://sse-relay.sundae.fi/publish",
        };
        Url::parse(url).unwrap()
    }
}

pub mod strategies {

    use balius_sdk::{
        Error,
        http::{HttpRequest, HttpResponse},
    };
    use tracing::info;

    use crate::{
        types::{
            serialize, Interval, Order, OutputReference, SignedStrategyExecution, StrategyExecution, SubmitSSE
        },
        utils::Network,
    };

    pub fn submit_execution(
        network: &Network,
        utxo: &OutputReference,
        validity_range: Interval,
        details: Order,
    ) -> Result<HttpResponse, Error> {
        let execution = StrategyExecution {
            tx_ref: utxo.clone(),
            validity_range,
            details,
            extensions: vec![],
        };

        let bytes = serialize(execution.clone());

        let signature = balius_sdk::wit::balius::app::sign::sign_payload("default", &bytes)?;

        let sse = SignedStrategyExecution {
            execution,
            signature: Some(signature),
        };
        let sse_bytes = serialize(sse);

        let submit_sse = SubmitSSE {
            tx_hash: hex::encode(&utxo.transaction_id.0),
            tx_index: utxo.output_index,
            data: hex::encode(&sse_bytes),
        };

        info!(
            "posting to {}: {} / {} / {}",
            network.relay_url(),
            hex::encode(&utxo.transaction_id.0),
            utxo.output_index,
            hex::encode(&sse_bytes)
        );

        Ok(HttpRequest::post(network.relay_url())
            .json(&submit_sse)?
            .send()?)
    }
}

pub mod keys {
    use crate::config::DCAConfig;
    use balius_sdk::{Config, Error, Json, Params, WorkerResult};
    use serde::Serialize;
    use std::collections::HashMap;

    pub fn get_signer_key(
        _: Config<DCAConfig>,
        _: Params<HashMap<String, String>>,
    ) -> WorkerResult<Json<SignerKey>> {
        let Some(key) = balius_sdk::get_public_keys().remove("default") else {
            return Err(Error::Internal("key not found".into()));
        };
        Ok(Json(SignerKey {
            signer: hex::encode(key),
        }))
    }

    #[derive(Serialize)]
    pub struct SignerKey {
        signer: String,
    }
}

pub mod kv {
    use balius_sdk::{WorkerResult, wit::balius::app::kv};
    use serde::{Deserialize, Serialize};

    /// Retrieve a value from the KV store. Returns None if the value does not already exist.
    pub fn get<D: for<'a> Deserialize<'a>>(key: &str) -> WorkerResult<Option<D>> {
        match kv::get_value(key) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(kv::KvError::NotFound(_)) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Store a value in the KV store.
    pub fn set<S: Serialize>(key: &str, value: &S) -> WorkerResult<()> {
        kv::set_value(key, &serde_json::to_vec(value)?)?;
        Ok(())
    }
}
