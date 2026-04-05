# Getting Started with Solana Recover

Welcome to Solana Recover! This guide will help you get up and running quickly, whether you're an individual user looking to recover SOL from unused token accounts or a developer integrating the tool into your applications.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Basic Usage](#basic-usage)
- [Configuration](#configuration)
- [Examples](#examples)
- [Next Steps](#next-steps)

## Prerequisites

### System Requirements

- **Operating System**: Windows 10+, macOS 10.15+, or Linux (Ubuntu 20.04+)
- **Memory**: Minimum 4GB RAM (8GB+ recommended for batch processing)
- **Storage**: 100MB free disk space
- **Network**: Internet connection for Solana RPC access

### Required Software

- **Rust**: Version 1.70.0 or newer
  ```bash
  # Install Rust (if not already installed)
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source ~/.cargo/env
  ```

- **Git**: For cloning the repository
  ```bash
  # Ubuntu/Debian
  sudo apt-get install git
  
  # macOS
  brew install git
  
  # Windows
  # Download from https://git-scm.com/
  ```

### Solana Knowledge

- Basic understanding of Solana wallet addresses
- Familiarity with SOL and lamports (1 SOL = 1,000,000,000 lamports)
- Knowledge of token accounts and rent recovery concepts

## Installation

### Option 1: Build from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/Genius740Code/Sol-account-cleaner.git
cd Sol-account-cleaner

# Build the project
cargo build --release

# Verify installation
./target/release/solana-recover --version
```

### Option 2: Install from Crates.io (Coming Soon)

```bash
# Install directly from crates.io
cargo install solana-recover

# Verify installation
solana-recover --version
```

### Option 3: Docker Installation

```bash
# Pull the Docker image
docker pull solana-recover:latest

# Run with Docker
docker run --rm -it solana-recover:latest --help
```

## Quick Start

Let's get you running with your first wallet scan in just a few minutes!

### 1. Scan Your First Wallet

```bash
# Scan a single wallet address
solana-recover scan 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM

# Output will show:
# - Total token accounts found
# - Empty accounts eligible for rent recovery
# - Estimated recoverable SOL
# - Fee calculation
```

### 2. Scan Multiple Wallets

```bash
# Create a file with wallet addresses (one per line)
echo "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM" > wallets.txt
echo "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" >> wallets.txt

# Scan all wallets in the file
solana-recover batch wallets.txt

# Results will be saved to ./results/ directory
```

### 3. Start the API Server

```bash
# Start the REST API server
solana-recover server --port 8080

# The server will start on http://localhost:8080
# Visit http://localhost:8080/health to check status
```

## Basic Usage

### Command Line Interface

The CLI provides several commands for different use cases:

#### Single Wallet Scan
```bash
solana-recover scan [OPTIONS] <ADDRESS>

# Options:
#   -f, --format <FORMAT>  Output format [json|table] [default: table]
#   -c, --config <FILE>   Configuration file path
#   --log-level <LEVEL>   Log level [trace|debug|info|warn|error]
```

#### Batch Processing
```bash
solana-recover batch [OPTIONS] <FILE>

# Options:
#   -o, --output <DIR>     Output directory [default: ./results]
#   -c, --config <FILE>    Configuration file path
#   --log-level <LEVEL>    Log level
```

#### API Server
```bash
solana-recover server [OPTIONS]

# Options:
#   -p, --port <PORT>      Server port [default: 8080]
#   -h, --host <HOST>      Bind address [default: 0.0.0.0]
#   -c, --config <FILE>    Configuration file path
```

### Understanding the Output

#### Single Wallet Scan Results

```
┌─────────────────────────────────────────────────────────────┐
│                    Wallet Scan Results                     │
├─────────────────────────────────────────────────────────────┤
│ Wallet Address: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM │
│ Total Accounts: 25                                          │
│ Empty Accounts: 8                                          │
│ Recoverable: 0.00203928 SOL                                │
│ Service Fee: 0.00030589 SOL (15%)                          │
│ Net Recovery: 0.00173339 SOL                               │
│ Scan Time: 1.25 seconds                                    │
└─────────────────────────────────────────────────────────────┘

Empty Account Addresses:
- AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef
- BcDeFgHiJkLmNoPqRsTuVwXyZ2345678901bcdef
- ...
```

#### JSON Output Format

```json
{
  "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "total_accounts": 25,
  "empty_accounts": 8,
  "recoverable_lamports": 2039280,
  "recoverable_sol": 0.00203928,
  "fee_percentage": 0.15,
  "fee_lamports": 305892,
  "net_recovery_lamports": 1733388,
  "net_recovery_sol": 0.00173339,
  "empty_account_addresses": [
    "AbCdEfGhIjKlMnOpQrStUvWxYz1234567890abcdef",
    "BcDeFgHiJkLmNoPqRsTuVwXyZ2345678901bcdef"
  ],
  "scan_time_ms": 1250
}
```

## Configuration

### Default Configuration

The tool works out-of-the-box with sensible defaults, but you can customize behavior using configuration files.

### Creating a Configuration File

Create a `config.toml` file in your project directory:

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4

[rpc]
endpoints = ["https://api.mainnet-beta.solana.com"]
pool_size = 10
timeout_ms = 5000
rate_limit_rps = 100

[scanner]
batch_size = 100
max_concurrent_wallets = 1000
retry_attempts = 3
retry_delay_ms = 1000

[fees]
default_percentage = 0.15
minimum_lamports = 1000000
waive_below_lamports = 10000000

[logging]
level = "info"
format = "pretty"
```

### Environment Variables

You can also configure using environment variables:

```bash
export SOLANA_RECOVER_RPC_ENDPOINTS="https://api.mainnet-beta.solana.com,https://solana-api.projectserum.com"
export SOLANA_RECOVER_LOG_LEVEL=debug
export SOLANA_RECOVER_BATCH_SIZE=50
export SOLANA_RECOVER_FEE_PERCENTAGE=0.10
```

### Configuration Priority

1. Command line arguments (highest priority)
2. Environment variables
3. Configuration file
4. Default values (lowest priority)

## Examples

### Example 1: Basic Wallet Recovery

```bash
# Scan your personal wallet
solana-recover scan YOUR_WALLET_ADDRESS --format json

# Save results to a file
solana-recover scan YOUR_WALLET_ADDRESS > wallet_scan.json
```

### Example 2: Batch Processing for Exchange

```bash
# Create a file with customer wallet addresses
cat > customer_wallets.txt << EOF
CUSTOMER_WALLET_1
CUSTOMER_WALLET_2
CUSTOMER_WALLET_3
EOF

# Process all wallets with custom output directory
solana-recover batch customer_wallets.txt --output ./customer_results

# Generate summary report
python scripts/generate_summary.py ./customer_results
```

### Example 3: API Integration

```bash
# Start the API server in background
solana-recover server --port 8080 &

# Scan wallet via API
curl -X POST http://localhost:8080/api/v1/scan \
  -H "Content-Type: application/json" \
  -d '{"wallet_address": "YOUR_WALLET_ADDRESS"}'

# Batch scan via API
curl -X POST http://localhost:8080/api/v1/batch-scan \
  -H "Content-Type: application/json" \
  -d '{
    "wallet_addresses": ["WALLET_1", "WALLET_2"],
    "fee_percentage": 0.15
  }'
```

### Example 4: High-Performance Configuration

For processing large numbers of wallets:

```toml
[scanner]
batch_size = 500
max_concurrent_wallets = 2000

[rpc]
endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana"
]
pool_size = 50
rate_limit_rps = 200
```

## Next Steps

### For Individual Users

1. **Scan Your Wallets**: Use the CLI to check your personal wallets
2. **Understand Fees**: Learn about the fee structure and recovery process
3. **Recover Funds**: Follow the recovery process to claim your SOL
4. **Monitor**: Regularly scan for new empty accounts

### For Developers

1. **API Integration**: Use the REST API for application integration
2. **Batch Processing**: Implement batch processing for multiple users
3. **Custom Configuration**: Optimize settings for your use case
4. **Monitoring**: Set up metrics and alerting
5. **Examples**: Check the `examples/` directory for code samples

### For Businesses

1. **White-Label Solution**: Customize the tool for your brand
2. **Enterprise Features**: Set up user management and billing
3. **Compliance**: Ensure regulatory compliance
4. **Support**: Plan for customer support and documentation

## Additional Resources

- [API Documentation](api.md) - Complete API reference
- [Configuration Guide](configuration.md) - Detailed configuration options
- [Deployment Guide](deployment.md) - Production deployment
- [Examples](../examples/) - Code examples and tutorials
- [Troubleshooting](troubleshooting.md) - Common issues and solutions

## Getting Help

- 📧 **Email**: support@solana-recover.com
- 💬 **Discord**: [Join our community](https://discord.gg/solana-recover)
- 🐛 **Issues**: [GitHub Issues](https://github.com/Genius740Code/Sol-account-cleaner/issues)
- 📖 **Documentation**: [docs.solana-recover.com](https://docs.solana-recover.com)

---

Congratulations! You're now ready to start recovering SOL from unused token accounts. Happy scanning!
