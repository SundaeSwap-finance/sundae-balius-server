mod config;
use std::time::Duration;

use balius_sdk::{Ack, Config, WorkerResult};
use config::Config as StrategyConfig;
use sundae_strategies::{
    ManagedStrategy, PoolState, Strategy, kv,
    types::{Interval, Order, asset_amount},
};
use tracing::info;

pub const INITIAL_PRICE_PREFIX: &str = "base_price:";
fn base_price_key(pool_ident: &String) -> String {
    format!("{INITIAL_PRICE_PREFIX}{pool_ident}")
}

fn on_new_pool_state(
    config: &Config<StrategyConfig>,
    pool_state: &PoolState,
    strategies: &Vec<ManagedStrategy>,
) -> WorkerResult<Ack> {
    let pool_price = pool_state.pool_datum.raw_price(&pool_state.utxo);
    let pool_ident = hex::encode(&pool_state.pool_datum.identifier);

    let base_price = kv::get::<f64>(base_price_key(&pool_ident).as_str())?.unwrap_or(0.0);

    info!(
        "pool update found, with price {} against base price {}",
        pool_price, base_price
    );

    if pool_price < base_price * (1. - config.percent) {
        info!(
            "price has fallen to {}, below the base price of {}. Triggering a sell order...",
            pool_price, base_price
        );
        for strategy in strategies {
            trigger_swap(
                config,
                config.network.to_unix_time(pool_state.slot),
                strategy,
                true,
            )?;
        }
    } else if pool_price > base_price * (1. + config.percent) {
        info!(
            "price has risen to {}, above the base price of {}. Triggering a buy order...",
            pool_price, base_price
        );
        for strategy in strategies {
            trigger_swap(
                config,
                config.network.to_unix_time(pool_state.slot),
                strategy,
                false,
            )?;
        }
    }

    Ok(Ack)
}

fn trigger_swap(
    config: &StrategyConfig,
    now: u64,
    strategy: &ManagedStrategy,
    token_a: bool,
) -> WorkerResult<Ack> {
    let valid_for = Duration::from_secs_f64(20. * 60.);
    let validity_range = Interval::inclusive_range(
        now - valid_for.as_millis() as u64,
        now + valid_for.as_millis() as u64,
    );

    let swap = if token_a {
        Order::Swap {
            offer: (
                config.token_a.policy_id.clone(),
                config.token_a.asset_name.clone(),
                asset_amount(&strategy.utxo, &config.token_a),
            ),
            min_received: (
                config.token_b.policy_id.clone(),
                config.token_b.asset_name.clone(),
                1,
            ),
        }
    } else {
        Order::Swap {
            offer: (
                config.token_b.policy_id.clone(),
                config.token_b.asset_name.clone(),
                asset_amount(&strategy.utxo, &config.token_b),
            ),
            min_received: (
                config.token_a.policy_id.clone(),
                config.token_a.asset_name.clone(),
                1,
            ),
        }
    };

    sundae_strategies::submit_execution(&config.network, &strategy.output, validity_range, swap)?;
    Ok(Ack)
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Strategy::<StrategyConfig>::new()
        .on_new_pool_state(on_new_pool_state)
        .worker()
}
