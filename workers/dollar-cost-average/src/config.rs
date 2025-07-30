use serde::Deserialize;

use sundae_strategies::{Network, types::AssetId};

#[derive(Deserialize)]
pub struct DCAConfig {
    pub network: Network,
    pub interval: u64,
    pub offer_token: AssetId,
    pub offer_amount: u64,
    pub receive_token: AssetId,
    pub receive_amount_min: u64,
}
