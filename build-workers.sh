ROOT="$(cd "$(dirname "$0")/" && pwd)"
pushd $ROOT
for worker in $(ls workers); do
    echo "Compile $worker"
    cargo run --bin balius-worker-builder -- \
        --source-dir "workers/$worker" \
        --target-file "balius-server/workers/$worker.wasm"
done
popd
