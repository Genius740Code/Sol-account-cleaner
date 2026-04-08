# Solana Recover

[![Crates.io](https://img.shields.io/crates/v/solana-recover.svg)](https://crates.io/crates/solana-recover)
[![Documentation](https://docs.rs/solana-recover/badge.svg)](https://docs.rs/solana-recover)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Solana wallet scanner and SOL recovery library for Rust. This crate provides a simple yet powerful API for scanning Solana wallets to find empty token accounts and recover SOL from them.

## 🚀 Features

- **Simple API**: Easy-to-use functions for quick wallet scanning
- **High Performance**: Optimized for scanning multiple wallets concurrently
- **Feature-based**: Only compile what you need with feature flags
- **Type Safe**: Full Rust type safety with comprehensive error handling
- **Async First**: Built on tokio for efficient asynchronous operations
- **Extensible**: Modular design allows for custom implementations

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
solana-recover = "1.0.2"
```

### Feature Flags

Use only the features you need to keep your binary small:

```toml
# Default: scanner + client
solana-recover = "1.0.2"

# Minimal - just core types
solana-recover = { version = "1.0.2", default-features = false }

# Scanner functionality only
solana-recover = { version = "1.0.2", default-features = false, features = ["scanner"] }

# Full feature set
solana-recover = { version = "1.0.2", features = ["full"] }
```

Available features:
- `scanner` - Core wallet scanning functionality
- `client` - HTTP client for external APIs  
- `api` - REST API server functionality
- `database` - Database persistence support
- `cache` - Advanced caching capabilities
- `metrics` - Prometheus metrics collection
- `security` - Enhanced security features
- `config` - Configuration file support
- `full` - Enables all features

## 🏁 Quick Start

### Basic Wallet Scan

```rust
use solana_recover::scan_wallet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Scan a wallet for empty accounts
    let result = scan_wallet("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", None).await?;
    
    println!("Wallet: {}", result.wallet_address);
    println!("Total accounts: {}", result.total_accounts);
    println!("Empty accounts: {}", result.empty_accounts.len());
    println!("Recoverable SOL: {}", result.recoverable_sol);
    println!("Scan time: {}ms", result.scan_time_ms);
    
    Ok(())
}
```

### Advanced Usage with Scanner

```rust
use solana_recover::{WalletScanner, ScanConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create scanner with custom configuration
    let config = ScanConfig {
        rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
        max_concurrent: 20,
        timeout_seconds: 60,
        enable_cache: true,
    };
    
    let scanner = WalletScanner::with_config(config).await?;
    
    // Scan multiple wallets
    let wallets = vec![
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    ];
    
    for wallet in wallets {
        let result = scanner.scan_wallet(wallet).await?;
        println!("{} has {} recoverable SOL", wallet, result.recoverable_sol);
    }
    
    Ok(())
}
```

### Batch Processing

```rust
use solana_recover::{BatchProcessor, BatchScanRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let processor = BatchProcessor::new().await?;
    
    let request = BatchScanRequest {
        wallet_addresses: vec![
            "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        ],
        fee_percentage: Some(0.15),
    };
    
    let results = processor.process_batch(request).await?;
    
    for result in results.results {
        println!("Wallet {}: {} SOL recoverable", 
                 result.wallet_address, 
                 result.recoverable_sol);
    }
    
    Ok(())
}
```

## 🔧 Configuration

### Environment Variables

```bash
# Default RPC endpoint
export SOLANA_RPC_ENDPOINT="https://api.mainnet-beta.solana.com"

# Request timeout in seconds
export SOLANA_TIMEOUT_SECONDS=30

# Maximum concurrent requests
export SOLANA_MAX_CONCURRENT=10

# Enable caching
export SOLANA_ENABLE_CACHE=true
```

### Configuration File (with `config` feature)

```toml
[scanner]
rpc_endpoint = "https://api.mainnet-beta.solana.com"
max_concurrent = 20
timeout_seconds = 60
enable_cache = true

[cache]
ttl_seconds = 300
max_size = 1000

[fees]
default_percentage = 0.15
minimum_lamports = 1000000
```

## 📊 Examples

### Command Line Interface

The CLI tool provides both simple and advanced commands for wallet scanning and SOL recovery:

#### **Simple Usage (Recommended)**

**Quick scan:**
```bash
solana-recover --wallet <ADDRESS>
```

**Scan and reclaim in one command:**
```bash
solana-recover --wallet <ADDRESS> --destination <DESTINATION>
```

**Developer mode (show detailed information):**
```bash
solana-recover --wallet <ADDRESS> --dev
# or short form
solana-recover --wallet <ADDRESS> -D
```

#### **Advanced Usage**

**Show total claimable SOL:**
```bash
solana-recover show --targets "wallet:addr1,addr2,addr3"
solana-recover show --targets "key:privkey1,privkey2"
# Show with detailed information
solana-recover show --targets "wallet:addr1,addr2,addr3" --dev
```

**Reclaim SOL:**
```bash
solana-recover reclaim --targets "wallet:addr1,addr2" --destination "destination_wallet_address"
solana-recover reclaim --targets "key:privkey1,privkey2" --destination "dest_wallet_address"
# Reclaim with detailed information
solana-recover reclaim --targets "wallet:addr1,addr2" --destination "dest_wallet_address" --dev
```

**Batch processing:**
```bash
solana-recover batch wallets.txt
# Batch with detailed information
solana-recover batch wallets.txt --dev
```

#### **Examples**

```bash
# Quick scan of a single wallet
solana-recover --wallet B7bQUSYnD56Vk7jEAqU4MWLJQ9LgVnKyWskivPhZQcHg

# Scan and immediately reclaim SOL
solana-recover --wallet B7bQUSYnD56Vk7jEAqU4MWLJQ9LgVnKyWskivPhZQcHg --destination 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM

# Show total from multiple wallets
solana-recover show --targets "wallet:B7bQUSYnD56Vk7jEAqU4MWLJQ9LgVnKyWskivPhZQcHg,9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM"

# Force reclaim without confirmation
solana-recover --wallet <ADDRESS> --destination <DEST> --force

# Developer mode - show wallet address and empty account details
solana-recover --wallet <ADDRESS> --dev
solana-recover --wallet <ADDRESS> -D
```

### Simple CLI Tool

```rust
// examples/simple_scan.rs
use solana_recover::scan_wallet;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <wallet_address>", args[0]);
        std::process::exit(1);
    }
    
    let wallet_address = &args[1];
    let result = scan_wallet(wallet_address, None).await?;
    
    println!("Scan Results:");
    println!("  Wallet: {}", result.wallet_address);
    println!("  Total Accounts: {}", result.total_accounts);
    println!("  Empty Accounts: {}", result.empty_accounts.len());
    println!("  Recoverable SOL: {:.9}", result.recoverable_sol);
    println!("  Scan Time: {}ms", result.scan_time_ms);
    
    Ok(())
}
```

Run with:
```bash
cargo run --example simple_scan -- 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
```

### SOL Recovery Example

```rust
// examples/recover_sol.rs
use solana_recover::{recover_sol, RecoveryRequest};
use uuid::Uuid;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let request = RecoveryRequest {
        id: Uuid::new_v4(),
        wallet_address: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
        empty_accounts: vec!["empty_account_address".to_string()],
        destination_address: "destination_address".to_string(),
        wallet_connection_id: Some("wallet_connection".to_string()),
        max_fee_lamports: Some(5_000_000),
        priority_fee_lamports: None,
        user_id: None,
        created_at: Utc::now(),
    };
    
    let result = recover_sol(&request, None).await?;
    println!("Recovered {} SOL", result.net_sol);
    
    Ok(())
}
```

### Web Server (with `api` feature)

```rust
// examples/web_server.rs
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use solana_recover::scan_wallet;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/scan", post(scan_wallet_handler));
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running on http://{}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

async fn scan_wallet_handler(Json(payload): Json<Value>) -> Result<Json<Value>, String> {
    let wallet_address = payload["wallet_address"]
        .as_str()
        .ok_or("Missing wallet_address")?;
    
    let rpc_endpoint = payload["rpc_endpoint"]
        .as_str();
    
    match scan_wallet(wallet_address, rpc_endpoint).await {
        Ok(result) => Ok(Json(json!({
            "success": true,
            "data": result
        }))),
        Err(e) => Ok(Json(json!({
            "success": false,
            "error": e.to_string()
        }))),
    }
}
```

## 🔍 Error Handling

The library provides comprehensive error handling with the `SolanaRecoverError` enum:

```rust
use solana_recover::{scan_wallet, SolanaRecoverError};

#[tokio::main]
async fn main() {
    match scan_wallet("invalid_address", None).await {
        Ok(result) => println!("Success: {:?}", result),
        Err(SolanaRecoverError::InvalidAddress(addr)) => {
            eprintln!("Invalid wallet address: {}", addr);
        }
        Err(SolanaRecoverError::NetworkError(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }
}
```

## 🧪 Testing

Run the test suite:

```bash
cargo test

# Run with specific features
cargo test --features "full"

# Run integration tests
cargo test --test integration
```

## 📚 Documentation

- [API Documentation](https://docs.rs/solana-recover)
- [Examples](https://github.com/your-org/solana-recover/tree/main/examples)
- [Guide](https://github.com/your-org/solana-recover/wiki)

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
git clone https://github.com/your-org/solana-recover.git
cd solana-recover
cargo build
cargo test
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

- 📧 Email: support@solana-recover.com
- 🐛 Issues: [GitHub Issues](https://github.com/your-org/solana-recover/issues)
- 💬 Discord: [Join our community](https://discord.gg/solana-recover)

## 🙏 Acknowledgments

- [Solana Labs](https://solana.com/) for the amazing blockchain platform
- The Rust community for excellent tooling and libraries

## ❓ Frequently Asked Questions

### Does the tool only close accounts with active positions or all accounts?

The tool specifically targets **empty token accounts** only. Here's what that means:

- **Empty Accounts**: Token accounts with a balance of 0 tokens but still holding rent exemption SOL (typically 2.228268 SOL per account)
- **Active Positions**: Accounts with non-zero token balances are **never** closed or touched
- **Safety First**: The scanner verifies each account has exactly 0 tokens before including it in recovery

**What gets recovered:**
- Rent exemption SOL from empty token accounts
- Transaction fees are deducted from the recovered amount
- Only accounts with 0 token balance are eligible

**What's safe:**
- Accounts with active token positions (any non-zero balance)
- SOL accounts (native SOL accounts)
- Accounts with delegated tokens or active stakes
- NFTs or other assets with non-zero balances

### Is it safe to use private keys with this tool?

Yes, but with important considerations:

- Private keys are only used to derive the public wallet address for scanning
- For recovery operations, private keys are used to sign transactions locally
- Keys are never transmitted to external services
- Always ensure you're using a secure environment and backup your keys

### What happens to the tokens in empty accounts?

Empty accounts by definition have 0 tokens. The "empty" refers to the token balance being 0, not the account being completely devoid of value. These accounts still hold rent exemption SOL that can be recovered.

### Can I recover SOL from accounts with small token balances?

No. The tool only processes accounts with exactly 0 token balance. Accounts with any non-zero token balance (even very small amounts) are skipped to preserve your token positions.

---

**Built with ❤️ for the Solana ecosystem**
