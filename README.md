# Bitcoin Transaction Broadcaster

A simple HTTP service for broadcasting Bitcoin transactions. This service provides an HTTP endpoint for submitting raw transactions to a Bitcoin node.

## Prerequisites

- Rust toolchain
- Running Bitcoin Core node with RPC access
- Access to Bitcoin node RPC credentials

## Configuration

The service can be configured using command-line arguments:

- `--network`: Bitcoin network type (bitcoin, testnet, regtest, signet) [default: regtest]
- `--bitcoin-url`: Bitcoin RPC URL [default: http://127.0.0.1]
- `--rpc-username`: Bitcoin RPC username [default: user]
- `--rpc-password`: Bitcoin RPC password [default: password]
- `--host`: Host address to bind the HTTP server [default: 127.0.0.1]
- `--port`: Port to bind the HTTP server [default: 5558]

The RPC port is automatically selected based on the network:
- Mainnet: 8332
- Testnet: 18332
- Regtest: 18443
- Signet: 38332

## Building and Running

The service includes several convenient commands using Just:

### Build the Service
```sh
just build
# or
just b
```

### Run with Default Settings
```shCopy
just run
```
### Run with Docker Configuration
```shCopy
just run-docker
```

### Run with Custom Settings
```shCopy
just run-custom <host> <port> <bitcoin_url> <network> <username> <password>
```

### API
#### Broadcast Transaction
```bashCopy
curl -X POST http://127.0.0.1:5558/broadcast \
  -H "Content-Type: application/json" \
  -d '{"raw_tx": "SIGNED_BTC_TX"}'
```
#### Response Format
```jsonCopy
{
    "status": "success" | "error",
    "txid": "transaction_id_bytes", // Base16 encoded transaction ID if successful
    "current_block": 123456,        // Current blockchain height
    "error": "error_message"        // Present only if status is "error"
}
```