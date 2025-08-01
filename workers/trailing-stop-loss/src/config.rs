use serde::Deserialize;
use sundae_strategies::{Network, types::AssetId};

#[derive(Deserialize)]
pub struct Config {
    pub network: Network,
    pub pool: String,
    pub token: AssetId,
    pub amount: u64,
    pub base_price: f64,
    pub trail_percent: f64,
    pub receive_token: AssetId,
}
