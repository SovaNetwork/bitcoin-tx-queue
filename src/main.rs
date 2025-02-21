use std::sync::Arc;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use bitcoin::Network;
use bitcoincore_rpc::{
    bitcoin::{hashes::Hash, Txid},
    Auth, Client, RpcApi,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
struct Args {
    /// Bitcoin network type (bitcoin, testnet, regtest, signet)
    #[arg(long, default_value = "regtest")]
    network: String,

    /// Bitcoin RPC URL
    #[arg(long, default_value = "http://127.0.0.1")]
    bitcoin_url: String,

    /// Bitcoin RPC username
    #[arg(long, default_value = "user")]
    rpc_username: String,

    /// Bitcoin RPC password
    #[arg(long, default_value = "password")]
    rpc_password: String,

    /// Host address to bind the HTTP server
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind the HTTP server
    #[arg(long, default_value = "5558")]
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastRequest {
    raw_tx: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastResponse {
    status: String,
    txid: Option<Vec<u8>>,
    current_block: u64,
    error: Option<String>,
}

struct BroadcastService {
    bitcoin_client: Arc<Client>,
}

impl BroadcastService {
    pub fn new(config: &BitcoinConfig) -> Result<Self, bitcoincore_rpc::Error> {
        let port = match config.network {
            Network::Bitcoin => 8332,
            Network::Testnet => 18332,
            Network::Regtest => 18443,
            Network::Signet => 38332,
            _ => unreachable!("unsupported network id"),
        };

        let url = format!("{}:{}", config.network_url, port);
        let auth = Auth::UserPass(config.rpc_username.clone(), config.rpc_password.clone());

        let client = Client::new(&url, auth)?;

        Ok(Self {
            bitcoin_client: Arc::new(client),
        })
    }

    fn broadcast_transaction(&self, raw_tx: &str) -> Result<Vec<u8>, bitcoincore_rpc::Error> {
        // call send_raw_transaction and get the txid in natural byte order
        let txid: Txid = self.bitcoin_client.send_raw_transaction(raw_tx)?;
        // Convert Txid to 32 byte vec and convert to reverse byte order.
        // Reverse byte order is the standard format for looking up btc txs in block explorer or node
        let mut bytes = txid.to_raw_hash().to_byte_array().to_vec();
        bytes.reverse();
        Ok(bytes)
    }

    fn get_current_block_height(&self) -> Result<u64, bitcoincore_rpc::Error> {
        self.bitcoin_client
            .get_block_count()
            .map(|height| height as u64)
    }
}

#[instrument(skip(service))]
async fn broadcast_transaction(
    service: web::Data<Arc<BroadcastService>>,
    req: web::Json<BroadcastRequest>,
) -> impl Responder {
    info!("Received broadcast request");

    let current_block = match service.get_current_block_height() {
        Ok(height) => height,
        Err(e) => {
            error!("Failed to get current block height: {}", e);
            return HttpResponse::InternalServerError().json(BroadcastResponse {
                status: "error".to_string(),
                txid: None,
                current_block: 0,
                error: Some(format!("Failed to get current block height: {}", e)),
            });
        }
    };

    match service.broadcast_transaction(&req.raw_tx) {
        Ok(txid) => {
            let mut txid_array = [0u8; 32];
            txid_array.copy_from_slice(&txid.clone());
            let hash = bitcoin::hashes::sha256d::Hash::from_bytes_ref(&txid_array);
            info!(
                "Successfully broadcast transaction: {}",
                bitcoin::Txid::from_raw_hash(hash.clone())
            );

            HttpResponse::Ok().json(BroadcastResponse {
                status: "success".to_string(),
                txid: Some(txid),
                current_block,
                error: None,
            })
        }
        Err(e) => {
            let error_msg = e.to_string();
            error!("Failed to broadcast transaction: {}", error_msg);
            HttpResponse::InternalServerError().json(BroadcastResponse {
                status: "error".to_string(),
                txid: None,
                current_block,
                error: Some(error_msg),
            })
        }
    }
}

#[derive(Clone)]
struct BitcoinConfig {
    network: Network,
    network_url: String,
    rpc_username: String,
    rpc_password: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting Bitcoin tx broadcast service...");

    // Parse network from string
    let network = match args.network.to_lowercase().as_str() {
        "bitcoin" => Network::Bitcoin,
        "testnet" => Network::Testnet,
        "regtest" => Network::Regtest,
        "signet" => Network::Signet,
        _ => {
            error!("Unsupported network: {}", args.network);
            return Ok(());
        }
    };

    // Initialize Bitcoin config with CLI args
    let config = BitcoinConfig {
        network,
        network_url: args.bitcoin_url,
        rpc_username: args.rpc_username,
        rpc_password: args.rpc_password,
    };

    // Create broadcast service
    let service =
        Arc::new(BroadcastService::new(&config).expect("Failed to create broadcast service"));

    // Start HTTP server with CLI-specified host and port
    let bind_address = format!("{}:{}", args.host, args.port);
    info!(
        "Starting HTTP Bitcoin broadcast tx server on {}",
        bind_address
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(service.clone()))
            .route("/broadcast", web::post().to(broadcast_transaction))
    })
    .bind(&bind_address)?
    .run()
    .await
}
