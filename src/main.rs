use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use bitcoin::Network;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use tracing_subscriber::EnvFilter;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
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

// Structs for the broadcast queue
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingTransaction {
    raw_tx: String,
    timestamp: DateTime<Utc>,
    attempts: u32,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastRequest {
    raw_tx: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastResponse {
    status: String,
    txid: Option<String>,
    error: Option<String>,
}

struct BroadcastService {
    bitcoin_client: Arc<Client>,
    pending_txs: Arc<RwLock<VecDeque<PendingTransaction>>>,
    max_retries: u32,
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
        let auth = Auth::UserPass(
            config.rpc_username.clone(),
            config.rpc_password.clone(),
        );

        let client = Client::new(&url, auth)?;

        Ok(Self {
            bitcoin_client: Arc::new(client),
            pending_txs: Arc::new(RwLock::new(VecDeque::new())),
            max_retries: 3,
        })
    }

    #[instrument(skip(self))]
    pub fn enqueue_transaction(&self, raw_tx: String) {
        info!("Enqueueing new transaction");
        
        let pending_tx = PendingTransaction {
            raw_tx,
            timestamp: Utc::now(),
            attempts: 0,
            last_error: None,
        };

        self.pending_txs.write().push_back(pending_tx);
        info!("Transaction queued successfully");
    }

    #[instrument(skip(self))]
    pub async fn process_pending_transactions(&self) {
        info!("Processing pending transactions");
        
        let mut txs_to_retry = VecDeque::new();
        let mut pending = self.pending_txs.write();

        while let Some(mut tx) = pending.pop_front() {
            if tx.attempts >= self.max_retries {
                error!(
                    "Transaction exceeded max retries. Raw tx: {}, Last error: {:?}",
                    tx.raw_tx, tx.last_error
                );
                continue;
            }

            match self.broadcast_transaction(&tx.raw_tx) {
                Ok(txid) => {
                    info!("Successfully broadcast transaction: {}", txid);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    error!("Failed to broadcast transaction: {}", error_msg);
                    
                    tx.attempts += 1;
                    tx.last_error = Some(error_msg);
                    txs_to_retry.push_back(tx);
                }
            }
        }

        // Add failed transactions back to the queue
        pending.extend(txs_to_retry);
        
        info!(
            "Finished processing transactions. {} remaining in queue",
            pending.len()
        );
    }

    fn broadcast_transaction(&self, raw_tx: &str) -> Result<String, bitcoincore_rpc::Error> {
        let txid = self.bitcoin_client.send_raw_transaction(raw_tx)?;
        Ok(txid.to_string())
    }
}

// HTTP handlers
#[instrument(skip(service))]
async fn enqueue_transaction(
    service: web::Data<Arc<BroadcastService>>,
    req: web::Json<BroadcastRequest>,
) -> impl Responder {
    info!("Received broadcast request");

    service.enqueue_transaction(req.raw_tx.clone());

    HttpResponse::Ok().json(BroadcastResponse {
        status: "queued".to_string(),
        txid: None,
        error: None,
    })
}

#[instrument(skip(service))]
async fn get_queue_status(
    service: web::Data<Arc<BroadcastService>>,
) -> impl Responder {
    let queue_size = service.pending_txs.read().len();
    
    HttpResponse::Ok().json(serde_json::json!({
        "pending_transactions": queue_size
    }))
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

    info!("Starting Bitcoin transaction broadcast service");

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
    let service = Arc::new(BroadcastService::new(&config).expect("Failed to create broadcast service"));
    let service_clone = service.clone();

    // Spawn background task to process transactions
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        
        loop {
            interval.tick().await;
            service_clone.process_pending_transactions().await;
        }
    });

    // Start HTTP server with CLI-specified host and port
    let bind_address = format!("{}:{}", args.host, args.port);
    info!("Starting HTTP server on {}", bind_address);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(service.clone()))
            .route("/broadcast", web::post().to(enqueue_transaction))
            .route("/status", web::get().to(get_queue_status))
    })
    .bind(&bind_address)?
    .run()
    .await
}