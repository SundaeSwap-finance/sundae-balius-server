use std::fmt::{self, Debug};

use balius_sdk::txbuilder::{codec::minicbor, plutus::BigInt};
use plutus_parser::AsPlutus;
use serde::{Deserialize, Serialize, de};

pub struct AssetId {
    pub policy_id: Vec<u8>,
    pub asset_name: Vec<u8>,
}

struct AssetIdVisitor;
impl<'de> de::Visitor<'de> for AssetIdVisitor {
    type Value = AssetId;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .write_str("a string representing an assetId, in the format hexPolicyId.hexAssetName")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let (policy_id, asset_name) = v.split_once(".").ok_or(E::custom("unexpected format"))?;
        Ok(AssetId {
            policy_id: hex::decode(policy_id).or(Err(E::custom("policyId was not hex encoded")))?,
            asset_name: hex::decode(asset_name)
                .or(Err(E::custom("assetName was not hex encoded")))?,
        })
    }
}
impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(AssetIdVisitor)
    }
}

#[derive(AsPlutus, Serialize, Deserialize, Debug, Clone)]
pub struct PoolDatum {
    pub identifier: Vec<u8>,
    pub assets: ((Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)),
    pub circulating_lp: BigInt,
    pub bid_fees_per_10_thousand: BigInt,
    pub ask_fees_per_10_thousand: BigInt,
    pub fee_manager: Option<MultisigScript>,
    pub market_open: BigInt,
    pub protocol_fees: BigInt,
}

#[derive(AsPlutus, Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize)]
pub struct SubmitSSE {
    pub tx_hash: String,
    pub tx_index: u64,
    pub data: String,
}

#[derive(AsPlutus, Serialize, Deserialize, Debug, Clone)]
pub enum MultisigScript {
    Signature { key_hash: Vec<u8> },
}

#[derive(AsPlutus, Serialize, Deserialize, Debug, Clone)]
pub enum Destination {
    #[variant = 1]
    Self_,
}

#[derive(AsPlutus, Clone, Serialize, Deserialize, Debug)]
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

#[derive(AsPlutus, Clone, Serialize, Deserialize, Debug)]
pub enum StrategyAuthorization {
    Signature { signer: Vec<u8> },
}

#[derive(AsPlutus, Clone, Serialize, Deserialize, Debug)]
pub struct TransactionId(pub Vec<u8>);

#[derive(AsPlutus, Clone, Serialize, Deserialize)]
pub struct OutputReference {
    pub transaction_id: TransactionId,
    pub output_index: u64,
}

impl Debug for OutputReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            format!(
                "{}#{}",
                hex::encode(&self.transaction_id.0),
                self.output_index
            )
            .as_str(),
        )
    }
}

#[derive(AsPlutus, Clone)]
pub struct Interval {
    pub lower_bound: IntervalBound,
    pub upper_bound: IntervalBound,
}

impl Interval {
    pub fn inclusive_range(lower_millis: u64, upper_millis: u64) -> Self {
        Self {
            lower_bound: IntervalBound {
                bound_type: IntervalBoundType::Finite(lower_millis),
                is_inclusive: true,
            },
            upper_bound: IntervalBound {
                bound_type: IntervalBoundType::Finite(upper_millis),
                is_inclusive: true,
            },
        }
    }
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

#[test]
pub fn test_strategy_serialization() {
    let (offer_policy_id, offer_asset_name) = (vec![], vec![]);
    let (receive_policy_id, receive_asset_name) = (
        hex::decode("99b071ce8580d6a3a11b4902145adb8bfd0d2a03935af8cf66403e15").unwrap(),
        hex::decode("534245525259").unwrap(),
    );

    let validity_range = Interval {
        lower_bound: IntervalBound {
            bound_type: IntervalBoundType::Finite(1752270497000u64),
            is_inclusive: true,
        },
        upper_bound: IntervalBound {
            bound_type: IntervalBoundType::Finite(1752272897000u64),
            is_inclusive: true,
        },
    };

    let swap = Order::Swap {
        offer: (offer_policy_id, offer_asset_name, 10000000),
        min_received: (receive_policy_id, receive_asset_name, 1),
    };

    let execution = StrategyExecution {
        tx_ref: OutputReference {
            transaction_id: TransactionId(
                hex::decode("da432ef16b7aa9b3972bdd42f86e6605c14444e75678f4e6fd75baa01168086f")
                    .unwrap(),
            ),
            output_index: 0,
        },
        validity_range,
        details: swap,
        extensions: vec![],
    };

    let bytes = serialize(execution.clone());
    let expected_bytes = hex::decode("d8799fd8799fd8799f5820da432ef16b7aa9b3972bdd42f86e6605c14444e75678f4e6fd75baa01168086fff00ffd8799fd8799fd87a9f1b00000197fb75e4e8ffd87a80ffd8799fd87a9f1b00000197fb9a83e8ffd87a80ffffd87a9f9f40401a00989680ff9f581c99b071ce8580d6a3a11b4902145adb8bfd0d2a03935af8cf66403e154653424552525901ffff40ff").unwrap();
    assert_eq!(bytes, expected_bytes);
}
