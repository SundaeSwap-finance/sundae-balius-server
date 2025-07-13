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
