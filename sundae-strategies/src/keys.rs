use balius_sdk::{Config, Error, Json, Params, WorkerResult};
use serde::Serialize;
use std::collections::HashMap;

pub fn get_signer_key<T>(
    _: Config<T>,
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
