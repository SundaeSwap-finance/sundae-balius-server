mod config;

use std::time::Duration;

use balius_sdk::{Ack, Config, Tx, WorkerResult};
use tracing::info;

use crate::config::DCAConfig;

use sundae_strategies::{
    ManagedOrder, Strategy,
    types::{Interval, Order},
};

fn on_each_tx(
    config: Config<DCAConfig>,
    tx: Tx,
    tracked_orders: Vec<ManagedOrder>,
) -> WorkerResult<Ack> {
    for seen in tracked_orders {
        let slots_elapsed = tx.block_slot - seen.slot;
        if slots_elapsed > config.interval {
            info!("{} slots elapsed, triggering a buy order", slots_elapsed);
            trigger_buy(&config, &tx, &seen)?;
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

fn trigger_buy(config: &config::DCAConfig, tx: &Tx, order: &ManagedOrder) -> WorkerResult<()> {
    let now = config.network.to_unix_time(tx.block_slot);
    let valid_for = Duration::from_secs_f64(20. * 60.);
    let validity_range = Interval::inclusive_range(
        now - valid_for.as_millis() as u64,
        now + valid_for.as_millis() as u64,
    );

    let swap = Order::Swap {
        offer: (
            config.offer_token.policy_id.clone(),
            config.offer_token.asset_name.clone(),
            config.offer_amount,
        ),
        min_received: (
            config.receive_token.policy_id.clone(),
            config.receive_token.asset_name.clone(),
            config.receive_amount_min,
        ),
    };

    sundae_strategies::submit_execution(&config.network, &order.output, validity_range, swap)?;

    Ok(())
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    info!("Dollar Cost Average worker starting!");

    Strategy::<DCAConfig>::new().on_each_tx(on_each_tx).worker()
}
