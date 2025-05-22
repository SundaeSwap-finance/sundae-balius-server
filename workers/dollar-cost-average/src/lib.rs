use balius_sdk::{
    Ack, Config, FnHandler, Json, Params, Tx, Utxo, UtxoMatcher, Worker, WorkerResult,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use utxorpc_spec::utxorpc::v1alpha::cardano::{Constr, PlutusData, big_int::BigInt, plutus_data};

#[derive(Deserialize)]
struct MyConfig {
    custom_hello: Option<String>,
    interval: u64,
}

#[derive(Deserialize)]
struct HelloRequest {
    name: String,
}

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

fn say_hello(
    config: Config<MyConfig>,
    req: Params<HelloRequest>,
) -> WorkerResult<Json<HelloResponse>> {
    let message = format!(
        "{}, {}",
        config.custom_hello.as_deref().unwrap_or("Hello"),
        req.name
    );
    Ok(Json(HelloResponse { message }))
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
        let height_passed = tx.block_height - seen.height;
        if height_passed > config.interval {
            info!("we should fuckin buy");
        } else {
            info!("not yet...");
        }
    }

    Ok(Ack)
}

fn process_order(_: Config<MyConfig>, utxo: Utxo<()>) -> WorkerResult<Ack> {
    info!(
        height = utxo.block_height,
        "utxo: {}.{}",
        hex::encode(&utxo.tx_hash),
        utxo.index
    );

    let Some(datum) = utxo.utxo.datum.and_then(|d| parse_order_datum(d.payload?)) else {
        return Ok(Ack);
    };
    let key = balius_sdk::wit::balius::app::sign::get_public_key("default", "ed25519")?;

    #[allow(irrefutable_let_patterns)]
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
        height: utxo.block_height,
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

mod kv {
    use balius_sdk::{WorkerResult, wit::balius::app::kv};
    use serde::{Deserialize, Serialize};

    /// Retrieve a value from the KV store. Returns None if the value does not already exist.
    pub fn get<D: for<'a> Deserialize<'a>>(key: &str) -> WorkerResult<Option<D>> {
        match kv::get_value(key) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(kv::KvError::NotFound(_)) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Store a value in the KV store.
    pub fn set<S: Serialize>(key: &str, value: &S) -> WorkerResult<()> {
        kv::set_value(key, &serde_json::to_vec(value)?)?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SeenOrderDetails {
    height: u64,
    tx_hash: Vec<u8>,
    index: u64,
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Worker::new()
        .with_request_handler("say-hello", FnHandler::from(say_hello))
        .with_tx_handler(UtxoMatcher::all(), FnHandler::from(process_tx))
        .with_utxo_handler(UtxoMatcher::all(), FnHandler::from(process_order))
}

#[allow(unused)]
struct OrderDatum {
    pool_ident: Option<Vec<u8>>,
    owner: MultisigScript,
    max_protocol_fee: BigInt,
    destination: Destination,
    details: Order,
    extra: PlutusData,
}

#[allow(unused)]
enum MultisigScript {
    Signature { key_hash: Vec<u8> },
}

enum Destination {
    Self_,
}

enum Order {
    Strategy { auth: StrategyAuthorization },
}

enum StrategyAuthorization {
    Signature { signer: Vec<u8> },
}

fn parse_order_datum(data: PlutusData) -> Option<OrderDatum> {
    let [
        pool_ident,
        owner,
        max_protocol_fee,
        destination,
        details,
        extra,
    ] = parse_variant(data, 0)?;

    let pool_ident = parse_option(pool_ident).map(|o| o.and_then(parse_bytes))?;
    let owner = {
        let [key_hash] = parse_variant(owner, 0)?;
        MultisigScript::Signature {
            key_hash: parse_bytes(key_hash)?,
        }
    };

    let max_protocol_fee = parse_int(max_protocol_fee)?;

    let destination = {
        let [] = parse_variant(destination, 1)?;
        Destination::Self_
    };

    let details = {
        let [auth] = parse_variant(details, 0)?;
        let [signer] = parse_variant(auth, 0)?;
        Order::Strategy {
            auth: StrategyAuthorization::Signature {
                signer: parse_bytes(signer)?,
            },
        }
    };

    Some(OrderDatum {
        pool_ident,
        owner,
        max_protocol_fee,
        destination,
        details,
        extra,
    })
}

fn parse_constr(data: PlutusData) -> Option<(u32, Vec<PlutusData>)> {
    let plutus_data::PlutusData::Constr(Constr { tag, fields, .. }) = data.plutus_data? else {
        return None;
    };
    Some((tag - 121, fields.to_vec()))
}

fn parse_variant<const N: usize>(data: PlutusData, tag: u32) -> Option<[PlutusData; N]> {
    let (real_tag, fields) = parse_constr(data)?;
    if real_tag != tag {
        return None;
    }
    fields.try_into().ok()
}

fn parse_bytes(data: PlutusData) -> Option<Vec<u8>> {
    let plutus_data::PlutusData::BoundedBytes(bytes) = data.plutus_data? else {
        return None;
    };
    Some(bytes.to_vec())
}

fn parse_int(data: PlutusData) -> Option<BigInt> {
    let plutus_data::PlutusData::BigInt(bigint) = data.plutus_data? else {
        return None;
    };
    bigint.big_int
}

fn parse_option(data: PlutusData) -> Option<Option<PlutusData>> {
    match parse_constr(data)? {
        (0, mut fields) if fields.len() == 1 => Some(fields.pop()),
        (1, fields) if fields.is_empty() => Some(None),
        _ => None,
    }
}
