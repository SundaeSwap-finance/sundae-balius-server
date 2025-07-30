pub mod keys;
pub mod kv;
pub mod types;

use url::Url;
use balius_sdk::{
    _internal::Handler,
    Ack, Config, Error, FnHandler, Tx, Utxo, UtxoMatcher, Worker, WorkerResult,
    http::{HttpRequest, HttpResponse},
    wit,
};
use serde::{Deserialize, Serialize};
use tracing::{info, trace};

use crate::{
    types::{
        Interval, Order, OrderDatum, OutputReference, SignedStrategyExecution,
        StrategyAuthorization, StrategyExecution, SubmitSSE, TransactionId, serialize,
    },
    keys::get_signer_key,
};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Preview,
    Mainnet,
}

impl Network {
    pub fn to_unix_time(&self, slot: u64) -> u64 {
        let offset = match self {
            Self::Preview => 1666656000,
            Self::Mainnet => 1591566291,
        };
        (slot + offset) * 1000
    }

    pub fn relay_url(&self) -> Url {
        let url = match self {
            Self::Preview => "http://sse-relay.preview.sundae.fi/publish",
            Self::Mainnet => "http://sse-relay.sundae.fi/publish",
        };
        Url::parse(url).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SeenOrderDetails {
    pub slot: u64,
    pub utxo: OutputReference,
    pub order: OrderDatum,
}

pub struct NewOrderCallback<T>(Option<fn(Config<T>, SeenOrderDetails) -> WorkerResult<Ack>>);
pub struct TxCallback<T>(Option<fn(Config<T>, Tx, Vec<SeenOrderDetails>) -> WorkerResult<Ack>>);
pub struct Strategy<T> {
    new_order_callback: NewOrderCallback<T>,
    tx_callback: TxCallback<T>,
}

impl<T: Send + Sync + 'static> Strategy<T>
where
    Config<T>: TryFrom<Vec<u8>, Error = balius_sdk::Error>,
{
    pub fn new() -> Self {
        Strategy {
            new_order_callback: NewOrderCallback(None),
            tx_callback: TxCallback(None),
        }
    }

    pub fn on_new_order(
        mut self,
        f: fn(Config<T>, SeenOrderDetails) -> WorkerResult<Ack>,
    ) -> Self {
        self.new_order_callback = NewOrderCallback(Some(f));
        self
    }
    pub fn on_each_tx(
        mut self,
        f: fn(Config<T>, Tx, Vec<SeenOrderDetails>) -> WorkerResult<Ack>,
    ) -> Self {
        self.tx_callback = TxCallback(Some(f));
        self
    }
    pub fn worker(self) -> Worker {
        let new_order = self.new_order_callback;
        let tx = self.tx_callback;
        Worker::new()
            .with_request_handler("get-signer-key", FnHandler::from(get_signer_key::<T>))
            .with_utxo_handler(UtxoMatcher::all(), new_order)
            .with_tx_handler(UtxoMatcher::all(), tx)
            .with_signer(STRATEGY_KEY, "ed25519")
    }
}

impl<T: Send + Sync + 'static> Handler for NewOrderCallback<T>
where
    Config<T>: TryFrom<Vec<u8>, Error = balius_sdk::Error>,
{
    fn handle(
        &self,
        config: wit::Config,
        event: wit::Event,
    ) -> Result<wit::Response, wit::HandleError> {
        let config: Config<T> = config.try_into()?;
        let utxo: Utxo<()> = event.try_into()?;
        trace!(
            slot = utxo.block_slot,
            tx_ref = format!("{}#{}", hex::encode(&utxo.tx_hash), utxo.index),
            "transaction output observed",
        );

        // Check if it's a sundae v3 order datum?
        let Some(datum) = utxo
            .utxo
            .datum
            .and_then(|d| types::try_parse::<OrderDatum>(&d.original_cbor))
        else {
            trace!(
                slot = utxo.block_slot,
                tx_ref = format!("{}#{}", hex::encode(&utxo.tx_hash), utxo.index),
                "transaction output not a sundae v3 order",
            );
            return Ok(Ack.try_into()?);
        };

        trace!(
            slot = utxo.block_slot,
            tx_ref = format!("{}#{}", hex::encode(&utxo.tx_hash), utxo.index),
            "transaction output is a sundae v3 order",
        );

        let Some(key) = balius_sdk::get_public_keys().remove(STRATEGY_KEY) else {
            return Err(Error::Internal("key not found".into()).into());
        };

        // Check if it's *our* order
        use Order::Strategy;
        use StrategyAuthorization::Signature;
        match &datum.details {
            Strategy {
                auth: Signature { signer },
            } => {
                if signer == &key {
                    info!("transaction output is a strategy order owned by us ({})", hex::encode(signer));
                } else {
                    info!("transaction output is a strategy order not owned by ({}), not us ({})", hex::encode(signer), hex::encode(&key));
                    return Ok(Ack.try_into()?);
                }
            }
            _ => {
                info!("transaction output is not a strategy order");
                return Ok(Ack.try_into()?)
            },
        }

        info!(
            slot = utxo.block_slot,
            tx_ref = format!("{}#{}", hex::encode(&utxo.tx_hash), utxo.index),
            "strategy order observed",
        );

        // Save this order in our key-value store
        let seen = SeenOrderDetails {
            slot: utxo.block_slot,
            utxo: OutputReference {
                transaction_id: TransactionId(utxo.tx_hash),
                output_index: utxo.index,
            },
            order: datum,
        };

        // This is an order "under our custody", so we hold onto it
        let mut all_seen: Vec<SeenOrderDetails> =
            kv::get(KV_MANAGED_ORDERS)?.unwrap_or_default();
        all_seen.push(seen.clone());
        kv::set(KV_MANAGED_ORDERS, &all_seen)?;

        info!("now tracking {} orders", all_seen.len());

        if let Some(callback) = self.0 {
            let response = callback(config, seen)?;
            Ok(response.try_into()?)
        } else {
            Ok(Ack.try_into()?)
        }
    }
}

impl<T: Send + Sync + 'static> Handler for TxCallback<T>
where
    Config<T>: TryFrom<Vec<u8>, Error = balius_sdk::Error>,
{
    fn handle(
        &self,
        config: wit::Config,
        event: wit::Event,
    ) -> Result<wit::Response, wit::HandleError> {
        let config: Config<T> = config.try_into()?;
        let tx: Tx = event.try_into()?;
        trace!(
            slot = tx.block_slot,
            tx_hash = hex::encode(&tx.hash),
            "transaction observed",
        );
        let spent_inputs = tx
            .tx
            .inputs
            .iter()
            .map(|input| (input.tx_hash.to_vec(), input.output_index as u64))
            .collect::<Vec<_>>();

        trace!("Marking orders as spent, if any...");
        let mut seen_orders: Vec<SeenOrderDetails> =
            kv::get(KV_MANAGED_ORDERS)?.unwrap_or_default();

        seen_orders.retain(|spent| {
            let (spent_hash, spent_index) =
                (&spent.utxo.transaction_id.0, &spent.utxo.output_index);
            !spent_inputs
                .iter()
                .any(|(hash, index)| spent_hash == hash && spent_index == index)
        });

        kv::set(KV_MANAGED_ORDERS, &seen_orders)?;

        trace!("remaining orders: {:?}", seen_orders);

        if let Some(callback) = self.0 {
            let response = callback(config, tx, seen_orders)?;
            Ok(response.try_into()?)
        } else {
            Ok(Ack.try_into()?)
        }
    }
}

pub const KV_MANAGED_ORDERS: &'static str = "managed_orders";
pub const STRATEGY_KEY: &'static str = "default";

pub fn submit_execution(
    network: &Network,
    utxo: &OutputReference,
    validity_range: Interval,
    details: Order,
) -> Result<HttpResponse, Error> {
    let execution = StrategyExecution {
        tx_ref: utxo.clone(),
        validity_range,
        details,
        extensions: vec![],
    };

    let bytes = serialize(execution.clone());

    let signature = balius_sdk::wit::balius::app::sign::sign_payload("default", &bytes)?;

    let sse = SignedStrategyExecution {
        execution,
        signature: Some(signature),
    };
    let sse_bytes = serialize(sse);

    let submit_sse = SubmitSSE {
        tx_hash: hex::encode(&utxo.transaction_id.0),
        tx_index: utxo.output_index,
        data: hex::encode(&sse_bytes),
    };

    info!(
        "posting to {}: {} / {} / {}",
        network.relay_url(),
        hex::encode(&utxo.transaction_id.0),
        utxo.output_index,
        hex::encode(&sse_bytes)
    );

    Ok(HttpRequest::post(network.relay_url())
        .json(&submit_sse)?
        .send()?)
}
