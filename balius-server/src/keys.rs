use std::path::PathBuf;

use anyhow::Result;
use pallas_crypto::key::ed25519::SecretKey;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Clone)]
pub struct KeyService {
    keys_dir: PathBuf,
}

impl KeyService {
    pub async fn new(keys_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&keys_dir).await?;
        Ok(Self { keys_dir })
    }

    pub async fn get_keys(&self, project_id: String) -> Result<Vec<(String, SecretKey)>> {
        let key_path = self.keys_dir.join(format!("{project_id}.json"));
        match fs::read(&key_path).await {
            Ok(bytes) => self.parse_keys(bytes),
            _ => self.create_keys(key_path).await,
        }
    }

    fn parse_keys(&self, bytes: Vec<u8>) -> Result<Vec<(String, SecretKey)>> {
        let raw: Vec<RawKey> = serde_json::from_slice(&bytes)?;
        Ok(raw
            .into_iter()
            .map(|raw| {
                let key = SecretKey::from(raw.key);
                (raw.name, key)
            })
            .collect())
    }

    async fn create_keys(&self, key_path: PathBuf) -> Result<Vec<(String, SecretKey)>> {
        let keys = vec![("default".to_string(), SecretKey::new(rand::thread_rng()))];

        let raw: Vec<RawKey> = keys
            .iter()
            .map(|(k, v)| RawKey {
                name: k.clone(),
                key: unsafe { SecretKey::leak_into_bytes(v.clone()) },
            })
            .collect();
        fs::write(key_path, serde_json::to_vec(&raw)?).await?;
        Ok(keys)
    }
}

#[derive(Serialize, Deserialize)]
struct RawKey {
    name: String,
    key: [u8; SecretKey::SIZE],
}
