mod config;

use std::time::Duration;

use balius_sdk::{Ack, Config, Tx, WorkerResult};
use tracing::info;

use crate::{
    config::DCAConfig,
};

use sundae_strategies::{
    types::{Interval, Order},
    SeenOrderDetails, Strategy
};


fn on_each_tx(
    config: Config<DCAConfig>,
    tx: Tx,
    orders: Vec<SeenOrderDetails>,
) -> WorkerResult<Ack> {
    for seen in orders {
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

fn trigger_buy(config: &config::DCAConfig, tx: &Tx, order: &SeenOrderDetails) -> WorkerResult<()> {
    let (offer_policy_id, offer_asset_name) = config.offer_token()?;
    let (receive_policy_id, receive_asset_name) = config.receive_token()?;

    let now = config.network.to_unix_time(tx.block_slot);
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

    sundae_strategies::submit_execution(&config.network, &order.utxo, validity_range, swap)?;

    Ok(())
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Strategy::<DCAConfig>::new().on_each_tx(on_each_tx).worker()
}
