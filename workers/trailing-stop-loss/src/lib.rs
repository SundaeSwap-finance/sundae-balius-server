mod config;

use std::time::Duration;

use balius_sdk::{Ack, Config, WorkerResult};
use config::Config as StrategyConfig;
use sundae_strategies::{
    ManagedStrategy, PoolState, Strategy, kv,
    types::{Interval, Order, asset_amount},
};
use tracing::info;

pub const BASE_PRICE_PREFIX: &str = "base_price:";
fn base_price_key(pool_ident: &String) -> String {
    format!("{BASE_PRICE_PREFIX}{pool_ident}")
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

    if pool_price < base_price {
        info!(
            "price has fallen to {}, below the base price of {}. Triggering a sell order...",
            pool_price, base_price,
        );
        for strategy in strategies {
            trigger_sell(
                config,
                config.network.to_unix_time(pool_state.slot),
                strategy,
            )?;
        }
    }

    let new_base_price: f64 = f64::max(base_price, pool_price * (1. - config.trail_percent));
    if new_base_price != base_price {
        info!("updating new base price to {}", new_base_price);
        kv::set(base_price_key(&pool_ident).as_str(), &new_base_price)?;
    }

    Ok(Ack)
}

fn trigger_sell(
    config: &StrategyConfig,
    now: u64,
    strategy: &ManagedStrategy,
) -> WorkerResult<Ack> {
    let valid_for = Duration::from_secs_f64(20. * 60.);
    let validity_range = Interval::inclusive_range(
        now - valid_for.as_millis() as u64,
        now + valid_for.as_millis() as u64,
    );

    let swap = Order::Swap {
        offer: (
            config.give_token.policy_id.clone(),
            config.give_token.asset_name.clone(),
            asset_amount(&strategy.utxo, &config.give_token),
        ),
        min_received: (
            config.receive_token.policy_id.clone(),
            config.receive_token.asset_name.clone(),
            1,
        ),
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
