# Solana Recover Examples

This directory contains comprehensive examples demonstrating how to use the Solana Recover library for various use cases.

## Examples Overview

### 1. Basic Scan (`basic_scan.rs`)
Demonstrates single wallet scanning with basic configuration.

**Features:**
- Single wallet scanning
- Fee calculation
- Result formatting
- Error handling

**Run:**
```bash
cargo run --example basic_scan
```

### 2. Batch Processing (`batch_processing.rs`)
Shows how to process multiple wallets efficiently with concurrent processing.

**Features:**
- Batch wallet scanning
- Concurrent processing
- Performance metrics
- Fee calculations for multiple wallets
- Detailed result analysis

**Run:**
```bash
cargo run --example batch_processing
```

### 3. Turnkey Integration (`turnkey_integration.rs`)
Enterprise wallet integration using Turnkey for secure key management.

**Features:**
- Turnkey wallet connection
- Enterprise fee structures
- Multiple wallet management
- Security best practices
- Transaction signing

**Run:**
```bash
cargo run --example turnkey_integration
```

### 4. API Client (`api_client.rs`)
HTTP client example for interacting with the Solana Recover API server.

**Features:**
- REST API integration
- Health checks
- Single and batch scanning via API
- Metrics retrieval
- Error handling

**Prerequisites:**
Start the API server first:
```bash
cargo run -- server
```

Then run the example:
```bash
cargo run --example api_client
```

## Configuration

All examples use the default configuration files in the `config/` directory. You can customize:

- `config/default.toml` - Default settings
- `config/development.toml` - Development environment
- `config/production.toml` - Production environment

## Common Patterns

### Error Handling
All examples demonstrate proper error handling:
```rust
match scanner.scan_wallet(address).await {
    Ok(result) => {
        // Handle success
    }
    Err(e) => {
        // Handle error
        eprintln!("Scan failed: {}", e);
    }
}
```

### Fee Calculation
```rust
let fee_structure = FeeStructure {
    percentage: 0.15, // 15%
    minimum_lamports: 1_000_000,
    maximum_lamports: Some(10_000_000),
    waive_below_lamports: Some(5_000_000),
};

let fee_calc = FeeCalculator::calculate_wallet_fee(&wallet_info, &fee_structure);
```

### Configuration Loading
```rust
let config = Config::load()?;
// Or with custom file
let config = Config::from_file("custom.toml")?;
```

### Logging Setup
```rust
let logging_config = LoggingConfig {
    level: "info".to_string(),
    format: LogFormat::Pretty,
    output: LogOutput::Stdout,
    file_path: None,
    json_fields: vec![],
};

Logger::init(logging_config)?;
```

## Performance Tips

1. **Batch Processing**: Use batch scanning for multiple wallets to improve efficiency
2. **Connection Pooling**: Configure appropriate pool sizes for your workload
3. **Rate Limiting**: Respect RPC provider rate limits
4. **Caching**: Enable caching for frequently accessed wallets
5. **Concurrent Processing**: Use appropriate concurrency levels

## Security Considerations

1. **API Keys**: Store API keys in environment variables, not in code
2. **Turnkey Integration**: Follow Turnkey security best practices
3. **Input Validation**: Always validate wallet addresses and parameters
4. **Error Logging**: Log errors without exposing sensitive information
5. **Network Security**: Use HTTPS in production environments

## Testing

Run all examples:
```bash
# Run individual examples
cargo run --example basic_scan
cargo run --example batch_processing
cargo run --example turnkey_integration
cargo run --example api_client

# Run with custom config
RUST_LOG=debug cargo run --example basic_scan -- --config config/development.toml
```

## Troubleshooting

### Common Issues

1. **Network Timeouts**: Increase timeout values in configuration
2. **RPC Errors**: Check RPC endpoint connectivity and rate limits
3. **Memory Usage**: Reduce batch sizes for large scans
4. **Permission Errors**: Ensure proper file permissions for data directories

### Debug Mode

Run examples with debug logging:
```bash
RUST_LOG=debug cargo run --example basic_scan
```

### Environment Variables

- `RUST_LOG`: Set logging level (trace, debug, info, warn, error)
- `DATABASE_URL`: Override database connection string
- `RPC_ENDPOINTS`: Override RPC endpoints (comma-separated)

## Next Steps

After running these examples, you can:

1. **Build Custom Applications**: Use these patterns in your own applications
2. **Integrate with APIs**: Build web services using the API client example
3. **Enterprise Integration**: Adapt the Turnkey example for enterprise use
4. **Performance Optimization**: Tune configuration based on your workload
5. **Monitoring**: Implement metrics and monitoring for production use

## Additional Resources

- [Main Documentation](../README.md)
- [API Documentation](../docs/api.md)
- [Configuration Guide](../docs/configuration.md)
- [Deployment Guide](../docs/deployment.md)
