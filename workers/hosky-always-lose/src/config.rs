use serde::Deserialize;
use sundae_strategies::{Network, types::AssetId};

#[derive(Deserialize)]
pub struct Config {
    pub network: Network,
    pub token_a: AssetId,
    pub token_b: AssetId,
    pub percent: f64,
}
