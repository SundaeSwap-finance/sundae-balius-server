mod types;
mod utils;

use std::time::Duration;

use balius_sdk::{
    Ack, Config, Error, FnHandler, Tx, Utxo, UtxoMatcher, Worker, WorkerResult, http::HttpRequest,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    types::{
        Interval, IntervalBound, Order, OrderDatum, OutputReference, SignedStrategyExecution,
        StrategyAuthorization, StrategyExecution,
    },
    utils::{Network, kv},
};

#[derive(Deserialize)]
struct MyConfig {
    network: Network,
    interval: u64,
    offer_token: String,
    offer_amount: u64,
    receive_token: String,
    receive_amount_min: u64,
    valid_for_secs: f64,
}

fn process_tx(config: Config<MyConfig>, tx: Tx) -> WorkerResult<Ack> {
    let spent_txos = tx
        .tx
        .inputs
        .iter()
        .map(|txi| (txi.tx_hash.to_vec(), txi.output_index as u64))
        .collect::<Vec<_>>();

    let mut seen_orders: Vec<SeenOrderDetails> = kv::get("seen_orders")?.unwrap_or_default();

    seen_orders.retain(|deets| {
        spent_txos
            .iter()
            .all(|(hash, index)| &deets.tx_hash != hash && &deets.index != index)
    });
    kv::set("seen_orders", &seen_orders)?;

    for seen in seen_orders {
        let slot_passed = tx.block_height - seen.slot;
        if slot_passed > config.interval {
            info!("trying to make a buy");
            buy_buy_buy(&config, &seen)?;
        } else {
            info!("not yet...");
        }
    }

    Ok(Ack)
}

fn buy_buy_buy(config: &MyConfig, order: &SeenOrderDetails) -> WorkerResult<()> {
    let Some((offer_policy_id, offer_asset_name)) = parse_token(&config.offer_token) else {
        return Err(Error::Internal(format!(
            "Invalid offer token {}",
            config.offer_token
        )));
    };
    let Some((receive_policy_id, receive_asset_name)) = parse_token(&config.offer_token) else {
        return Err(Error::Internal(format!(
            "Invalid receive token {}",
            config.receive_token
        )));
    };

    let now = config.network.to_unix_time(order.slot);
    let valid_for = Duration::from_secs_f64(config.valid_for_secs);
    let validity_range = Interval {
        lower_bound: IntervalBound {
            bound_type: types::IntervalBoundType::Finite(now),
            is_inclusive: true,
        },
        upper_bound: IntervalBound {
            bound_type: types::IntervalBoundType::Finite(now + valid_for.as_millis() as u64),
            is_inclusive: true,
        },
    };

    let swap = Order::Swap {
        offer: (offer_policy_id, offer_asset_name, config.offer_amount),
        min_received: (
            receive_policy_id,
            receive_asset_name,
            config.receive_amount_min,
        ),
    };

    let execution = StrategyExecution {
        tx_ref: OutputReference {
            transaction_id: order.tx_hash.clone(),
            output_index: order.index,
        },
        validity_range,
        details: swap,
        extensions: vec![],
    };

    let bytes = types::serialize(execution.clone());

    let signature = balius_sdk::wit::balius::app::sign::sign_payload("default", "ed25519", &bytes)?;

    let sse = SignedStrategyExecution {
        execution,
        signature,
    };
    let sse_bytes = types::serialize(sse);

    let submit_sse = SubmitSSE {
        tx_hash: hex::encode(&order.tx_hash),
        tx_index: order.index,
        data: hex::encode(sse_bytes),
    };
    HttpRequest::post(config.network.relay_url())
        .json(&submit_sse)?
        .send()?;
    Ok(())
}

fn parse_token(token: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let (policy_id, asset_name) = token.split_once(".")?;
    Some((hex::decode(policy_id).ok()?, hex::decode(asset_name).ok()?))
}

#[derive(Serialize)]
struct SubmitSSE {
    tx_hash: String,
    tx_index: u64,
    data: String,
}

fn process_order(_: Config<MyConfig>, utxo: Utxo<()>) -> WorkerResult<Ack> {
    info!(
        height = utxo.block_height,
        "utxo: {}.{}",
        hex::encode(&utxo.tx_hash),
        utxo.index
    );

    let Some(datum) = utxo
        .utxo
        .datum
        .and_then(|d| types::try_parse::<OrderDatum>(&d.original_cbor))
    else {
        return Ok(Ack);
    };
    let key = balius_sdk::wit::balius::app::sign::get_public_key("default", "ed25519")?;

    if let Order::Strategy {
        auth: StrategyAuthorization::Signature { signer },
    } = &datum.details
    {
        if signer != &key {
            return Ok(Ack);
        }
    } else {
        return Ok(Ack);
    }

    let seen = SeenOrderDetails {
        slot: utxo.block_slot,
        tx_hash: utxo.tx_hash,
        index: utxo.index,
    };

    // track this for l8r
    let mut all_seen: Vec<SeenOrderDetails> = kv::get("seen_orders")?.unwrap_or_default();
    all_seen.push(seen);
    kv::set("seen_orders", &all_seen)?;
    info!("seen_orders: {all_seen:?}");

    Ok(Ack)
}

#[derive(Debug, Serialize, Deserialize)]
struct SeenOrderDetails {
    slot: u64,
    tx_hash: Vec<u8>,
    index: u64,
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Worker::new()
        .with_tx_handler(UtxoMatcher::all(), FnHandler::from(process_tx))
        .with_utxo_handler(UtxoMatcher::all(), FnHandler::from(process_order))
}
