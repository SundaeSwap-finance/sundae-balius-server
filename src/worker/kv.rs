use async_trait::async_trait;
use balius_runtime::wit::balius::app::kv as wit;
use std::collections::BTreeMap;

pub struct InMemory {
    data: BTreeMap<String, Vec<u8>>,
}

impl InMemory {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }
}

#[async_trait]
impl wit::Host for InMemory {
    async fn list_values(&mut self, prefix: String) -> Result<Vec<String>, wit::KvError> {
        Ok(self
            .data
            .range((prefix.clone())..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .map(|(k, _)| k.clone())
            .collect())
    }

    async fn get_value(&mut self, key: String) -> Result<Vec<u8>, wit::KvError> {
        match self.data.get(&key) {
            Some(value) => Ok(value.clone()),
            None => Err(wit::KvError::NotFound(key)),
        }
    }

    async fn set_value(&mut self, key: String, value: Vec<u8>) -> Result<(), wit::KvError> {
        self.data.insert(key, value);
        Ok(())
    }
}
