# Sundae Balius Server

A quick-and-dirty server to run Balius workers.

## Setup

Create a `balius-server/config.yaml` file. See [balius-server/config.example.yaml](balius-server/config.example.yaml) for required settings.

```sh
# compile all the WASM which comes bundled with the server
./build-workers.sh

# run the server
cargo run --bin balius-server -- -c ./balius-server/config.yaml

# instantiate a dollar-cost-average worker through the server's API
./dollar-cost-average.sh
```
