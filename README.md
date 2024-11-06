# Bitcoin Transaction Broadcaster

A service for Bitcoin transaction broadcasting with automatic retry functionality. This service provides a queuing system for Bitcoin transactions and exposes HTTP endpoints for transaction submission and queue monitoring.

## Prerequisites

- Rust toolchain
- Running Bitcoin Core node with RPC access
- Access to Bitcoin node RPC credentials

## Configuration

The service requires the following configuration:

- Bitcoin network selection (Mainnet, Testnet, Regtest, Signet)
- Bitcoin Core RPC URL
- RPC username
- RPC password

Default configuration uses:
- Network: Regtest
- URL: http://127.0.0.1
- Port: Based on network (Mainnet: 8332, Testnet: 18332, Regtest: 18443)

### Build and Run the Service
Run the following command to build and start the service:
```sh
cargo run --release
```
or if you have Just installed:

```sh
just run
```
The service will now be running at http://localhost:5558.

## API Endpoints

### Submit Transaction
```bash
curl -X POST http://127.0.0.1:5558/broadcast \
  -H "Content-Type: application/json" \
  -d '{"raw_tx": "SIGNED_BTC_TX"}'
```

### Check Queue Status
```bash
curl http://127.0.0.1:5558/status
```

## How It Works

1. Transactions are submitted via the HTTP API
2. Each transaction is queued with metadata including submission time and retry count
3. A background worker processes the queue every 10 seconds
4. Failed transactions are automatically retried up to 3 times
5. Transactions exceeding the retry limit are logged and dropped