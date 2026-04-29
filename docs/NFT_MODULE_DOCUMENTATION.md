# NFT Module Documentation

## Overview

The NFT module provides a comprehensive, ultra-fast, and highly customizable NFT analysis and management system for the Solana Recover project. It offers sub-second wallet scanning, advanced portfolio analysis, security validation, and intelligent valuation capabilities.

## Features

### 🚀 Performance
- **Sub-second scanning**: Ultra-fast NFT discovery and analysis
- **Parallel processing**: Work-stealing and adaptive resource management
- **Intelligent caching**: Multi-tier caching with smart eviction policies
- **Batch optimization**: Efficient processing of multiple NFTs/wallets

### 🔒 Security
- **Comprehensive validation**: Multi-layered security assessment
- **Threat intelligence**: Integration with threat detection systems
- **Blacklist management**: Dynamic blocking of suspicious entities
- **Risk assessment**: Detailed risk scoring and recommendations

### 💰 Valuation
- **Multiple valuation methods**: Floor price, recent sales, rarity-based
- **Market data integration**: Real-time market intelligence
- **Portfolio analysis**: Comprehensive portfolio metrics and insights
- **Performance tracking**: Historical valuation and trend analysis

### 🎛️ Customization
- **Pluggable strategies**: Customizable processing workflows
- **Flexible configuration**: Extensive configuration options
- **Performance modes**: UltraFast, Fast, Balanced, Thorough modes
- **Extensible architecture**: Easy to add new features and integrations

## Architecture

### Core Components

1. **NftScanner**: Main entry point for NFT operations
2. **MetadataFetcher**: Ultra-fast metadata fetching and validation
3. **ValuationEngine**: Comprehensive NFT valuation system
4. **SecurityValidator**: Advanced security validation and risk assessment
5. **PortfolioAnalyzer**: Deep portfolio analysis and insights
6. **BatchProcessor**: High-performance batch processing
7. **CacheManager**: Multi-tier caching system
8. **StrategyManager**: Pluggable strategy system

### Module Structure

```
src/nft/
├── mod.rs              # Module exports and feature gates
├── types.rs            # Core type definitions
├── errors.rs           # Comprehensive error handling
├── scanner.rs          # Main NFT scanner
├── metadata.rs         # Metadata fetching and validation
├── valuation.rs        # Valuation engine
├── security.rs         # Security validation
├── portfolio.rs        # Portfolio analysis
├── batch.rs           # Batch processing
├── cache.rs           # Caching system
├── strategies.rs      # Processing strategies
└── tests/             # Module tests
```

## Quick Start

### Installation

Enable the NFT feature in your `Cargo.toml`:

```toml
[dependencies]
solana-recover = { version = "1.0.7", features = ["nft"] }
```

### Basic Usage

```rust
use solana_recover::{scan_wallet_nfts, NftScanResult};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Scan a wallet for NFTs with balanced analysis
    let result: NftScanResult = scan_wallet_nfts(
        "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
        None
    ).await?;
    
    println!("Found {} NFTs", result.nfts.len());
    println!("Total value: {} SOL", 
        result.total_estimated_value_lamports as f64 / 1_000_000_000.0);
    println!("Security issues: {}", result.security_issues.len());
    
    Ok(())
}
```

### Ultra-Fast Scanning

For maximum performance when you need quick results:

```rust
use solana_recover::scan_wallet_nfts_ultra_fast;

let result = scan_wallet_nfts_ultra_fast(
    "wallet_address_here",
    Some("https://api.mainnet-beta.solana.com")
).await?;
```

### Thorough Analysis

For comprehensive analysis with maximum validation:

```rust
use solana_recover::scan_wallet_nfts_thorough;

let result = scan_wallet_nfts_thorough(
    "wallet_address_here",
    None
).await?;
```

### Batch Processing

Scan multiple wallets efficiently:

```rust
use solana_recover::scan_wallets_nfts_batch;

let wallets = vec![
    "wallet1_address".to_string(),
    "wallet2_address".to_string(),
    "wallet3_address".to_string(),
];

let results = scan_wallets_nfts_batch(&wallets, None).await?;
for result in results {
    println!("Wallet {}: {} NFTs", result.wallet_address, result.nfts.len());
}
```

## Performance Modes

### UltraFast Mode
- **Use case**: Quick scans, real-time applications
- **Features**: Minimal validation, maximum parallelization
- **Performance**: Sub-second scanning for typical wallets
- **Trade-offs**: Less comprehensive analysis

### Fast Mode
- **Use case**: Regular scanning with good performance
- **Features**: Basic validation, optimized caching
- **Performance**: 1-2 seconds for typical wallets
- **Trade-offs**: Moderate analysis depth

### Balanced Mode (Default)
- **Use case**: General-purpose scanning
- **Features**: Comprehensive analysis, good performance
- **Performance**: 2-5 seconds for typical wallets
- **Trade-offs**: Balanced approach

### Thorough Mode
- **Use case**: Security-critical applications
- **Features**: Maximum validation, deep analysis
- **Performance**: 5-15 seconds for typical wallets
- **Trade-offs**: Slower but most comprehensive

## Configuration

### Scanner Configuration

```rust
use solana_recover::nft::{NftScanner, NftScannerConfig, PerformanceMode};

let config = NftScannerConfig {
    performance_mode: PerformanceMode::UltraFast,
    enable_metadata_fetching: true,
    enable_valuation: true,
    enable_security_validation: true,
    enable_portfolio_analysis: true,
    max_concurrent_scans: 20,
    scan_timeout_seconds: 60,
    max_nfts_per_wallet: Some(1000),
    ..Default::default()
};

let scanner = NftScanner::new(connection_pool, config)?;
```

### Cache Configuration

```rust
use solana_recover::nft::cache::CacheConfig;

let cache_config = CacheConfig {
    l1_max_entries: 10000,
    l2_max_entries: 50000,
    l1_ttl_seconds: 300,      // 5 minutes
    l2_ttl_seconds: 1800,     // 30 minutes
    enable_compression: true,
    compression_threshold_bytes: 1024,
    enable_intelligent_eviction: true,
    ..Default::default()
};
```

### Security Configuration

```rust
use solana_recover::nft::security::SecurityValidatorConfig;

let security_config = SecurityValidatorConfig {
    enable_comprehensive_validation: true,
    enable_metadata_validation: true,
    enable_image_validation: true,
    enable_creator_validation: true,
    strict_mode: false,
    block_high_risk: false,
    min_confidence_threshold: 0.7,
    enable_threat_intel: true,
    ..Default::default()
};
```

## Data Types

### NftInfo

Comprehensive NFT information:

```rust
pub struct NftInfo {
    pub id: Uuid,
    pub mint_address: String,
    pub token_account: String,
    pub owner: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub metadata_uri: Option<String>,
    pub image_uri: Option<String>,
    pub collection: Option<CollectionInfo>,
    pub creators: Vec<CreatorInfo>,
    pub attributes: Vec<NftAttribute>,
    pub estimated_value_lamports: Option<u64>,
    pub security_assessment: SecurityAssessment,
    pub rarity_score: Option<f64>,
    pub quality_score: Option<f64>,
    // ... additional fields
}
```

### NftScanResult

Complete scan results:

```rust
pub struct NftScanResult {
    pub scan_id: Uuid,
    pub wallet_address: String,
    pub nfts: Vec<NftInfo>,
    pub portfolio: Option<NftPortfolio>,
    pub security_issues: Vec<SecurityIssue>,
    pub total_estimated_value_lamports: u64,
    pub statistics: ScanStatistics,
    pub performance: ScanPerformanceMetrics,
    pub scanned_at: DateTime<Utc>,
    pub scan_duration_ms: u64,
}
```

### SecurityAssessment

Security evaluation:

```rust
pub struct SecurityAssessment {
    pub risk_level: RiskLevel,        // None, Low, Medium, High, Critical
    pub security_score: u8,           // 0-100, higher is more secure
    pub issues: Vec<SecurityIssue>,
    pub verified: bool,
    pub assessed_at: DateTime<Utc>,
    pub confidence: u8,               // 0-100
}
```

## Security Features

### Risk Assessment

The security system provides comprehensive risk assessment with multiple levels:

- **None**: No security risks detected
- **Low**: Minor concerns, generally safe
- **Medium**: Caution advised, investigate further
- **High**: Dangerous, avoid interaction
- **Critical**: Extremely dangerous, block immediately

### Security Rules

Built-in security validation rules:

1. **SuspiciousMetadata**: Detects suspicious metadata URIs and patterns
2. **UnverifiedCreator**: Identifies unverified or suspicious creators
3. **BrokenMetadata**: Detects incomplete or broken metadata
4. **SuspiciousDomain**: Identifies malicious or suspicious domains
5. **CopymintDetection**: Detects potential copymints and fakes

### Threat Intelligence

Integration with threat intelligence systems:

- Address blacklisting
- Domain reputation checking
- Collection threat assessment
- Community reporting integration

## Valuation Methods

### Floor Price Valuation

Uses collection floor prices as a baseline:

```rust
pub struct FloorPriceValuationStrategy {
    // Adjusts for rarity and other factors
    pub rarity_multiplier: f64,
    pub base_value: u64,
}
```

### Recent Sales Valuation

Analyzes recent sales data:

```rust
pub struct RecentSalesValuationStrategy {
    // Weighted average of recent sales
    pub time_decay_factor: f64,
    pub min_sales_count: u32,
    pub max_age_days: u32,
}
```

### Rarity-Based Valuation

Values NFTs based on rarity scores:

```rust
pub struct RarityValuationStrategy {
    // Rarity-based multipliers
    pub ultra_rare_multiplier: f64,  // 95+ rarity score
    pub very_rare_multiplier: f64,   // 85-94 rarity score
    pub rare_multiplier: f64,        // 70-84 rarity score
}
```

## Portfolio Analysis

### Portfolio Metrics

Comprehensive portfolio analysis provides:

```rust
pub struct NftPortfolio {
    pub total_value_lamports: u64,
    pub total_count: u32,
    pub verified_count: u32,
    pub high_risk_count: u32,
    pub collection_breakdown: HashMap<String, CollectionBreakdown>,
    pub value_distribution: ValueDistribution,
    pub risk_distribution: RiskDistribution,
    pub quality_metrics: PortfolioQualityMetrics,
}
```

### Insights Generation

The system generates actionable insights:

- **Risk insights**: Security concerns and recommendations
- **Value insights**: Valuation opportunities and trends
- **Diversification insights**: Portfolio balance recommendations
- **Performance insights**: Historical performance analysis

## Caching System

### Multi-Tier Architecture

- **L1 Cache (Hot)**: Moka-based in-memory cache for frequently accessed data
- **L2 Cache (Warm)**: DashMap-based cache for larger datasets
- **Intelligent Eviction**: Priority-based considering access patterns
- **Compression**: Automatic compression for large entries

### Cache Keys

Typed cache keys for different data types:

```rust
// NFT metadata
CacheKey::metadata("mint_address")

// Valuation results
CacheKey::valuation("mint_address")

// Security validation
CacheKey::security("mint_address")

// Collection data
CacheKey::collection("collection_id")
```

## Batch Processing

### Adaptive Batching

The system adapts batch sizes based on:

- Available memory usage
- CPU utilization
- Network conditions
- Historical performance

### Work-Stealing Parallelism

Efficient parallel processing using Rayon:

```rust
// Automatic load balancing
let results: Vec<Result> = items
    .par_iter()
    .map(|item| process_item(item))
    .collect();
```

### Progress Tracking

Real-time progress reporting for long-running operations:

```rust
pub struct ProgressReport {
    pub completed_items: usize,
    pub total_items: usize,
    pub progress_percent: f64,
    pub estimated_remaining_seconds: Option<f64>,
    pub current_rate: f64,
}
```

## Error Handling

### Comprehensive Error Types

```rust
pub enum NftError {
    Configuration { message: String },
    Network { message: String, source: String },
    Metadata { message: String, mint_address: Option<String> },
    Validation { message: String, field: Option<String> },
    Security { message: String, risk_level: RiskLevel },
    Valuation { message: String, method: Option<String> },
    Cache { message: String, operation: Option<String> },
    Batch { message: String, failed_items: Option<u32> },
    // ... additional error types
}
```

### Recovery Strategies

Automatic error recovery with configurable strategies:

- **Retry with backoff**: For transient errors
- **Use alternative**: For service failures
- **Skip and continue**: For non-critical errors
- **Fallback methods**: For degraded functionality

## Performance Optimization

### Connection Pooling

Optimized RPC connection management:

- Health checks and circuit breakers
- Load balancing across endpoints
- Connection reuse and lifecycle management
- Rate limiting and timeout handling

### Memory Management

Efficient memory usage:

- Object pooling for frequent allocations
- Buffer management for network operations
- Automatic garbage collection
- Memory usage monitoring and optimization

### Parallel Processing

Maximum CPU utilization:

- Work-stealing thread pools
- Controlled concurrency with semaphores
- Lock-free data structures
- Adaptive resource management

## Integration Examples

### With Existing Solana Recovery

```rust
use solana_recover::{
    scan_wallet,           // SOL recovery
    scan_wallet_nfts,      // NFT scanning
    ConnectionPool,
};

// Scan for both SOL and NFTs
let sol_result = scan_wallet("wallet_address", None).await?;
let nft_result = scan_wallet_nfts("wallet_address", None).await?;

println!("Recoverable SOL: {}", sol_result.recoverable_sol);
println!("NFT count: {}", nft_result.nfts.len());
println!("NFT value: {} SOL", 
    nft_result.total_estimated_value_lamports as f64 / 1_000_000_000.0);
```

### Custom Strategy Implementation

```rust
use solana_recover::nft::strategies::*;

pub struct CustomValuationStrategy {
    // Custom configuration
}

#[async_trait]
impl NftProcessingStrategy for CustomValuationStrategy {
    fn name(&self) -> &str { "CustomValuation" }
    
    fn applies_to(&self, context: &StrategyContext) -> bool {
        context.options.enable_valuation && 
        context.performance_mode == PerformanceMode::Balanced
    }
    
    async fn execute(&self, context: StrategyContext) -> NftResult<StrategyResult> {
        // Custom valuation logic
        Ok(StrategyResult {
            strategy_name: self.name().to_string(),
            status: StrategyStatus::Success,
            // ... result fields
        })
    }
}
```

## Monitoring and Metrics

### Performance Metrics

Comprehensive metrics tracking:

```rust
pub struct ScannerMetrics {
    pub total_scans: AtomicU64,
    pub successful_scans: AtomicU64,
    pub avg_scan_time_ms: AtomicU64,
    pub total_nfts_processed: AtomicU64,
    pub cache_hit_rate: AtomicF64,
    pub security_issues_found: AtomicU64,
}
```

### Cache Statistics

Detailed cache performance:

```rust
pub struct CacheStats {
    pub total_requests: u64,
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub hit_ratio: f64,
    pub memory_usage_mb: u64,
    pub avg_access_time_us: u64,
}
```

## Best Practices

### Performance Optimization

1. **Use appropriate performance modes** for your use case
2. **Enable caching** for repeated operations
3. **Use batch processing** for multiple NFTs/wallets
4. **Monitor metrics** to identify bottlenecks
5. **Adjust concurrency** based on available resources

### Security Considerations

1. **Enable security validation** for unknown collections
2. **Review security issues** before interacting with NFTs
3. **Use threat intelligence** when available
4. **Implement custom rules** for specific requirements
5. **Regular updates** of security rules and blacklists

### Error Handling

1. **Use proper error types** for different scenarios
2. **Implement retry logic** for transient failures
3. **Provide fallback options** for degraded functionality
4. **Log errors appropriately** for debugging
5. **Handle partial failures** gracefully

## Troubleshooting

### Common Issues

1. **Slow scanning performance**
   - Check network connectivity
   - Verify RPC endpoint health
   - Adjust concurrency settings
   - Enable caching

2. **High memory usage**
   - Reduce batch sizes
   - Enable compression
   - Adjust cache limits
   - Monitor memory metrics

3. **Security validation failures**
   - Check internet connectivity
   - Verify threat intelligence service
   - Review security rules
   - Update blacklists

4. **Valuation inaccuracies**
   - Verify market data sources
   - Check collection verification
   - Review valuation methods
   - Update market data regularly

### Debug Mode

Enable debug logging for troubleshooting:

```rust
use tracing_subscriber;

// Initialize logging
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

// Debug scanning
let result = scan_wallet_nfts_debug(wallet_address, None).await?;
```

## API Reference

### Core Functions

- `scan_wallet_nfts_ultra_fast()` - Ultra-fast NFT scanning
- `scan_wallet_nfts()` - Balanced NFT scanning
- `scan_wallet_nfts_thorough()` - Thorough NFT scanning
- `scan_wallets_nfts_batch()` - Batch wallet scanning
- `fetch_nft_metadata()` - Metadata fetching only

### Configuration Types

- `NftScannerConfig` - Main scanner configuration
- `CacheConfig` - Caching system configuration
- `SecurityValidatorConfig` - Security validation configuration
- `ValuationEngineConfig` - Valuation engine configuration

### Data Types

- `NftInfo` - Complete NFT information
- `NftScanResult` - Scan results with analysis
- `SecurityAssessment` - Security evaluation
- `NftPortfolio` - Portfolio analysis
- `ValuationResult` - Valuation details

## Contributing

### Adding New Features

1. **Create feature branch** from main
2. **Implement feature** with proper tests
3. **Update documentation** with examples
4. **Add integration tests** for new functionality
5. **Submit pull request** with detailed description

### Code Style

- Follow Rust best practices and idioms
- Use proper error handling with `Result` types
- Include comprehensive documentation
- Add unit and integration tests
- Maintain performance benchmarks

### Testing

```bash
# Run all tests
cargo test --features nft

# Run integration tests
cargo test --features nft --test integration

# Run performance benchmarks
cargo bench --features nft
```

## License

This NFT module is part of the Solana Recover project and is licensed under the MIT License. See the main project LICENSE file for details.

## Support

For support, questions, or contributions:

1. **Documentation**: Check this documentation and code comments
2. **Issues**: Open an issue on the project GitHub repository
3. **Discussions**: Use GitHub Discussions for questions
4. **Examples**: See the `examples/` directory for usage examples

---

*This documentation covers the comprehensive NFT module for the Solana Recover project. For the most up-to-date information, please refer to the source code and inline documentation.*
