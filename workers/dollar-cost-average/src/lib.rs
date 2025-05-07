use balius_sdk::{
    Ack, Config, FnHandler, Json, Params, Utxo, Worker, WorkerResult,
    wit::balius::app::driver::UtxoPattern,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Deserialize)]
struct MyConfig {
    custom_hello: Option<String>,
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

fn log_input(_: Config<MyConfig>, utxo: Utxo<()>) -> WorkerResult<Ack> {
    info!(height = utxo.block_height, "utxo: {}.{}", hex::encode(utxo.tx_hash), utxo.index);
    Ok(Ack)
}

#[balius_sdk::main]
fn main() -> Worker {
    balius_sdk::logging::init();

    Worker::new()
        .with_request_handler("say-hello", FnHandler::from(say_hello))
        .with_utxo_handler(
            UtxoPattern {
                address: None,
                token: None,
            },
            FnHandler::from(log_input),
        )
}
