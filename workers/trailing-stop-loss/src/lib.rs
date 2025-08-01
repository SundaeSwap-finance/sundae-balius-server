mod config;

use balius_sdk::txbuilder::plutus::BigInt;
use std::time::Duration;

use balius_sdk::{Ack, Config, Tx, WorkerResult};
use config::Config as StrategyConfig;
use sundae_strategies::{
    SeenOrderDetails, Strategy, kv,
    types::{self, Interval, Order, PoolDatum},
};
use tracing::info;
use utxorpc_spec::utxorpc::v1alpha::cardano::TxOutput;

pub const KEY: &'static str = "base_price";

// We need to:
// - Find pool update transactions
// - Determine token price
// - Compare to base price
// - if % increase >= step_percent, increase base price
// - If price falls below that (by some margin?) sell

fn on_each_tx(
    config: Config<StrategyConfig>,
    tx: Tx,
    strategies: Vec<SeenOrderDetails>,
) -> WorkerResult<Ack> {
    let base_price = match kv::get::<f64>(KEY)? {
        Some(base_price) => base_price,
        None => {
            kv::set(KEY, &config.base_price)?;
            config.base_price
        }
    };

    info!("checking for pool output in tx {}", hex::encode(&tx.hash));
    let mut maybe_pool_update: Option<(&TxOutput, PoolDatum)> = None;
    for (idx, output) in tx.tx.outputs.iter().enumerate() {
        info!("checking output {}", idx);
        if let Some(datum) = &output.datum && !datum.original_cbor.is_empty() {
            info!("output has a datum");
            match types::parse::<PoolDatum>(&datum.original_cbor) {
                Ok(pool_datum) => {
                    info!("parsed as pool datum {:?}", pool_datum);
                    if hex::encode(&pool_datum.identifier) == config.pool {
                        maybe_pool_update = Some((&output, pool_datum.clone()));
                        break;
                    }
                },
                Err(e) => {
                    info!("failed to parse: {:?}", e)
                },
            }
        }
    }

    if let Some((pool_output, pool_datum)) = maybe_pool_update {
        let price = token_price(pool_output, &pool_datum);
        info!("pool update found, with price {}", price);

        if price < base_price {
            info!(
                "price has fallen to {}, below the base price of {}. Triggering a sell order...",
                price, base_price,
            );
            for strategy in strategies {
                return trigger_sell(&config, &tx, &strategy, price);
            }
        }

        let new_base_price: f64 = f64::max(base_price, price * (1. - config.trail_percent));
        if new_base_price != base_price {
            info!("updating new base price to {}", new_base_price);
            let _ = kv::set(KEY, &new_base_price)?;
        }
    }

    Ok(Ack)
}

fn trigger_sell(
    config: &StrategyConfig,
    tx: &Tx,
    strategy: &SeenOrderDetails,
    price: f64,
) -> WorkerResult<Ack> {
    let now = config.network.to_unix_time(tx.block_slot);
    let valid_for = Duration::from_secs_f64(20. * 60.);
    let validity_range = Interval::inclusive_range(
        now - valid_for.as_millis() as u64,
        now + valid_for.as_millis() as u64,
    );

    let swap = Order::Swap {
        offer: (
            config.token.policy_id.clone(),
            config.token.asset_name.clone(),
            config.amount,
        ),
        min_received: (
            config.receive_token.policy_id.clone(),
            config.receive_token.asset_name.clone(),
            // We need to truncate here, since we can't get a partial coin
            (price * config.amount as f64) as u64,
        ),
    };

    sundae_strategies::submit_execution(&config.network, &strategy.utxo, validity_range, swap)?;
    Ok(Ack)
}

// There should be utility functions to do some common logic on an output (or on a pool output even?)
fn token_price(pool_output: &TxOutput, pool_datum: &PoolDatum) -> f64 {
    let assets: Vec<_> = pool_output
        .assets
        .iter()
        .flat_map(|multiasset| {
            multiasset.assets.iter().map(|asset| {
                (
                    (multiasset.policy_id.to_vec(), asset.name.to_vec()),
                    asset.output_coin,
                )
            })
        })
        .collect();

    // ADA?
    if pool_datum.assets.0.0.is_empty() && pool_datum.assets.0.1.is_empty() {
        (pool_output.coin - to_u64(&pool_datum.protocol_fees).unwrap()) as f64
            / assets
                .iter()
                .find(|(asset, _)| asset.0 == pool_datum.assets.1.0 && asset.1 == pool_datum.assets.1.1)
                .unwrap()
                .1 as f64
    } else {
        assets
            .iter()
            .find(|(asset, _)| asset.0 == pool_datum.assets.0.0 && asset.1 == pool_datum.assets.0.1)
            .unwrap()
            .1 as f64
            / assets
                .iter()
                .find(|(asset, _)| asset.0 == pool_datum.assets.1.0 && asset.1 == pool_datum.assets.1.1)
                .unwrap()
                .1 as f64
    }
}

fn to_u64(big_int: &BigInt) -> Option<u64> {
    match big_int {
        BigInt::Int(int) => u64::try_from(int.0).ok(),
        BigInt::BigUInt(_) | BigInt::BigNInt(_) => None,
    }
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Strategy::<StrategyConfig>::new()
        .on_each_tx(on_each_tx)
        .worker()
}
