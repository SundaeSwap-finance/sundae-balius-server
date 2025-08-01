use serde::Deserialize;
use sundae_strategies::{Network, types::AssetId};

#[derive(Deserialize)]
pub struct Config {
    pub network: Network,
    pub give_token: AssetId,
    pub receive_token: AssetId,
    pub trail_percent: f64,
}
