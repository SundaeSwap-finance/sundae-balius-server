use balius_sdk::Error;
use serde::Deserialize;

use sundae_strategies::Network;

#[derive(Deserialize)]
pub struct DCAConfig {
    pub network: Network,
    pub interval: u64,
    pub offer_token: String,
    pub offer_amount: u64,
    pub receive_token: String,
    pub receive_amount_min: u64,
}

fn parse_token(token: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let (policy_id, asset_name) = token.split_once(".")?;
    Some((hex::decode(policy_id).ok()?, hex::decode(asset_name).ok()?))
}

impl DCAConfig {
    pub fn offer_token(&self) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let Some((policy, name)) = parse_token(&self.offer_token) else {
            return Err(Error::Internal(format!(
                "Invalid offer token {}",
                self.offer_token
            )));
        };
        Ok((policy, name))
    }

    pub fn receive_token(&self) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let Some((policy, name)) = parse_token(&self.receive_token) else {
            return Err(Error::Internal(format!(
                "Invalid receive token {}",
                self.receive_token
            )));
        };
        Ok((policy, name))
    }
}
