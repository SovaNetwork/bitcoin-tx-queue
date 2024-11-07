# build rust binary
alias b := build

build:
    cargo build --release

# Run locally with default settings
run:
    RUST_LOG=info cargo run --release

# Run with Docker configuration (useful for testing Docker settings locally)
run-docker:
    RUST_LOG=info cargo run --release -- \
    --host 0.0.0.0 \
    --port 5558 \
    --bitcoin-url http://bitcoin-regtest \
    --network regtest \
    --rpc-username user \
    --rpc-password password

# Run with custom settings (usage: just run-custom host port bitcoin_url network username password)
run-custom host port bitcoin_url network username password:
    RUST_LOG=info cargo run --release -- \
    --host {{host}} \
    --port {{port}} \
    --bitcoin-url {{bitcoin_url}} \
    --network {{network}} \
    --rpc-username {{username}} \
    --rpc-password {{password}}
