mod config;
mod types;
mod utils;

use std::time::Duration;

use std::fmt::Debug;
use balius_sdk::{Ack, Config, Error, FnHandler, Tx, Utxo, UtxoMatcher, Worker, WorkerResult};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    types::{Interval, Order, OrderDatum, OutputReference, StrategyAuthorization, TransactionId},
    utils::{keys::get_signer_key, kv},
};

pub const KV_SEEN_ORDERS: &'static str = "seen_orders";

fn watch_for_orders(_: Config<config::DCAConfig>, utxo: Utxo<()>) -> WorkerResult<Ack> {
    info!(
        slot = utxo.block_slot,
        "utxo observed: {}.{}",
        hex::encode(&utxo.tx_hash),
        utxo.index
    );

    // Check if it's a sundae v3 order datum?
    let Some(datum) = utxo
        .utxo
        .datum
        .and_then(|d| types::try_parse::<OrderDatum>(&d.original_cbor))
    else {
        return Ok(Ack);
    };

    let Some(key) = balius_sdk::get_public_keys().remove("default") else {
        return Err(Error::Internal("key not found".into()));
    };

    // Check if it's *our* order
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

    // Save this order in our key-value store
    let seen = SeenOrderDetails {
        slot: utxo.block_slot,
        utxo: OutputReference {
            transaction_id: TransactionId(utxo.tx_hash),
            output_index: utxo.index,
        },
    };

    // This is an order "under our custody", so we hold onto it
    let mut all_seen: Vec<SeenOrderDetails> = kv::get(KV_SEEN_ORDERS)?.unwrap_or_default();
    all_seen.push(seen);
    kv::set(KV_SEEN_ORDERS, &all_seen)?;

    info!("tracking orders: {all_seen:?}");

    Ok(Ack)
}

fn process_tx(config: Config<config::DCAConfig>, tx: Tx) -> WorkerResult<Ack> {
    info!("processing tx {}", hex::encode(tx.hash));
    let spent_txos = tx
        .tx
        .inputs
        .iter()
        .map(|input| (input.tx_hash.to_vec(), input.output_index as u64))
        .collect::<Vec<_>>();

    info!("Marking orders as spent, if any...");
    let mut seen_orders: Vec<SeenOrderDetails> = kv::get(KV_SEEN_ORDERS)?.unwrap_or_default();

    seen_orders.retain(|spent| {
        let (spent_hash, spent_index) = (&spent.utxo.transaction_id.0, &spent.utxo.output_index);
        !spent_txos
            .iter()
            .any(|(hash, index)| spent_hash == hash && spent_index == index)
    });

    kv::set(KV_SEEN_ORDERS, &seen_orders)?;

    info!("retained orders: {:?}", seen_orders);

    for seen in seen_orders {
        let slots_elapsed = tx.block_slot - seen.slot;
        if slots_elapsed > config.interval {
            info!("{} slots elapsed, triggering a buy order", slots_elapsed);
            trigger_buy(&config, &seen)?;
        } else {
            info!(
                "{} slots elapsed, out of {}; {} slots remaining before we trigger a buy...",
                slots_elapsed,
                config.interval,
                config.interval - slots_elapsed
            );
        }
    }

    Ok(Ack)
}

fn trigger_buy(config: &config::DCAConfig, order: &SeenOrderDetails) -> WorkerResult<()> {
    let (offer_policy_id, offer_asset_name) = config.offer_token()?;
    let (receive_policy_id, receive_asset_name) = config.receive_token()?;

    let now = config.network.to_unix_time(order.slot);
    let valid_for = Duration::from_secs_f64(20. * 60.);
    let validity_range = Interval::inclusive_range(
        now - valid_for.as_millis() as u64,
        now + valid_for.as_millis() as u64,
    );

    let swap = Order::Swap {
        offer: (offer_policy_id, offer_asset_name, config.offer_amount),
        min_received: (
            receive_policy_id,
            receive_asset_name,
            config.receive_amount_min,
        ),
    };

    utils::strategies::submit_execution(&config.network, &order.utxo, validity_range, swap)?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct SeenOrderDetails {
    slot: u64,
    utxo: OutputReference,
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Worker::new()
        .with_request_handler("get-signer-key", FnHandler::from(get_signer_key))
        .with_utxo_handler(UtxoMatcher::all(), FnHandler::from(watch_for_orders))
        .with_tx_handler(UtxoMatcher::all(), FnHandler::from(process_tx))
        .with_signer("default", "ed25519")
}
