use balius_sdk::txbuilder::{codec::minicbor, plutus::BigInt};
use plutus_parser::AsPlutus;

#[derive(AsPlutus)]
pub struct OrderDatum {
    pub pool_ident: Option<Vec<u8>>,
    pub owner: MultisigScript,
    pub max_protocol_fee: BigInt,
    pub destination: Destination,
    pub details: Order,
    pub extra: Vec<u8>,
}

#[derive(AsPlutus)]
pub struct SignedStrategyExecution {
    pub execution: StrategyExecution,
    pub signature: Option<Vec<u8>>,
}

#[derive(AsPlutus, Clone)]
pub struct StrategyExecution {
    pub tx_ref: OutputReference,
    pub validity_range: Interval,
    pub details: Order,
    pub extensions: Vec<u8>,
}

#[derive(AsPlutus)]
pub enum MultisigScript {
    Signature { key_hash: Vec<u8> },
}

#[derive(AsPlutus)]
pub enum Destination {
    #[variant = 1]
    Self_,
}

#[derive(AsPlutus, Clone)]
pub enum Order {
    Strategy {
        auth: StrategyAuthorization,
    },
    Swap {
        offer: SingletonValue,
        min_received: SingletonValue,
    },
}

pub type SingletonValue = (Vec<u8>, Vec<u8>, u64);

#[derive(AsPlutus, Clone)]
pub enum StrategyAuthorization {
    Signature { signer: Vec<u8> },
}

#[derive(AsPlutus, Clone)]
pub struct TransactionId(pub Vec<u8>);

#[derive(AsPlutus, Clone)]
pub struct OutputReference {
    pub transaction_id: TransactionId,
    pub output_index: u64,
}

#[derive(AsPlutus, Clone)]
pub struct Interval {
    pub lower_bound: IntervalBound,
    pub upper_bound: IntervalBound,
}

#[derive(AsPlutus, Clone)]
pub struct IntervalBound {
    pub bound_type: IntervalBoundType,
    pub is_inclusive: bool,
}

#[derive(AsPlutus, Clone)]
pub enum IntervalBoundType {
    NegativeInfinity,
    Finite(u64),
    PositiveInfinity,
}

pub fn try_parse<T: AsPlutus>(bytes: &[u8]) -> Option<T> {
    let data = minicbor::decode(bytes).ok()?;
    T::from_plutus(data).ok()
}

pub fn serialize<T: AsPlutus>(value: T) -> Vec<u8> {
    let mut bytes = vec![];
    minicbor::encode(value.to_plutus(), &mut bytes).expect("infallible");
    bytes
}
