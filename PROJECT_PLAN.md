# Solana Scalable Wallet Scanner - Project Plan

## Overview
Build a scalable, fast, and secure system that finds and manages open Solana wallets, capable of handling 1000s of wallets per user with support for Turnkey integration and wallet connections.

## Architecture Goals
- **Scalability**: Process 1000+ wallets concurrently per user
- **Performance**: Multi-threaded processing with connection pooling
- **Security**: Secure wallet integration with Turnkey and other providers
- **Efficiency**: Batch processing and rate limiting
- **Modularity**: Clean separation of concerns

## Project Structure

```
solana_recover/
├── Cargo.toml                    # Dependencies and project config
├── README.md                     # Project documentation
├── config/
│   ├── default.toml             # Default configuration
│   ├── development.toml         # Dev environment config
│   └── production.toml          # Production config
├── src/
│   ├── main.rs                  # Application entry point
│   ├── lib.rs                   # Library exports
│   ├── config/
│   │   ├── mod.rs              # Config module
│   │   └── settings.rs         # Configuration structures
│   ├── core/
│   │   ├── mod.rs              # Core module
│   │   ├── scanner.rs          # Main wallet scanning logic
│   │   ├── processor.rs        # Batch processing engine
│   │   └── types.rs            # Core data types
│   ├── rpc/
│   │   ├── mod.rs              # RPC module
│   │   ├── pool.rs             # Connection pool management
│   │   ├── client.rs           # RPC client wrapper
│   │   └── rate_limiter.rs     # Rate limiting
│   ├── wallet/
│   │   ├── mod.rs              # Wallet module
│   │   ├── manager.rs          # Wallet connection manager
│   │   ├── turnkey.rs          # Turnkey integration
│   │   ├── phantom.rs          # Phantom wallet support
│   │   └── solflare.rs         # Solflare wallet support
│   ├── storage/
│   │   ├── mod.rs              # Storage module
│   │   ├── cache.rs            # In-memory caching
│   │   └── persistence.rs      # Database persistence
│   ├── api/
│   │   ├── mod.rs              # API module
│   │   ├── server.rs           # HTTP API server
│   │   ├── handlers.rs         # API route handlers
│   │   └── middleware.rs       # API middleware
│   ├── utils/
│   │   ├── mod.rs              # Utilities module
│   │   ├── logging.rs          # Logging setup
│   │   ├── metrics.rs          # Performance metrics
│   │   └── validation.rs       # Input validation
│   └── cli/
│       ├── mod.rs              # CLI module
│       └── commands.rs         # CLI command handlers
├── tests/
│   ├── integration/            # Integration tests
│   └── unit/                   # Unit tests
├── examples/
│   ├── basic_scan.rs           # Basic scanning example
│   ├── batch_processing.rs     # Batch processing example
│   └── turnkey_integration.rs  # Turnkey integration example
└── docs/
    ├── api.md                  # API documentation
    ├── deployment.md           # Deployment guide
    └── architecture.md         # Architecture details
```

## File Details

### Core Files

#### `src/main.rs`
- Application entry point
- CLI argument parsing
- Service initialization
- Graceful shutdown handling

#### `src/config/settings.rs`
- Configuration structures using serde
- Environment-specific settings
- Validation and defaults
- Hot reload capability

#### `src/core/scanner.rs`
- Main wallet scanning engine
- Multi-threaded wallet processing
- Empty account detection logic
- SOL recovery calculations

#### `src/core/processor.rs`
- Batch processing coordinator
- Work queue management
- Parallel execution using rayon
- Result aggregation

#### `src/core/types.rs`
- Core data structures
- Wallet information types
- Scan result types
- Error definitions

### Infrastructure Files

#### `src/rpc/pool.rs`
- RPC connection pool implementation
- Connection lifecycle management
- Load balancing across endpoints
- Health checking

#### `src/rpc/client.rs`
- RPC client wrapper with retry logic
- Error handling and backoff
- Request/response logging
- Performance monitoring

#### `src/rpc/rate_limiter.rs`
- Rate limiting implementation
- Token bucket algorithm
- Per-endpoint limits
- Dynamic adjustment

### Wallet Integration Files

#### `src/wallet/manager.rs`
- Wallet connection management
- Multi-provider support
- Connection state tracking
- Authentication handling

#### `src/wallet/turnkey.rs`
- Turnkey API integration
- Secure key management
- Transaction signing
- Policy enforcement

#### `src/wallet/phantom.rs`
- Phantom wallet connection
- Browser integration
- Message signing
- Account management

#### `src/wallet/solflare.rs`
- Solflare wallet support
- Mobile integration
- Hardware wallet support
- Multi-account handling

### Storage Files

#### `src/storage/cache.rs`
- In-memory caching using dashmap
- TTL-based expiration
- LRU eviction policy
- Cache statistics

#### `src/storage/persistence.rs`
- Database abstraction layer
- Query optimization
- Transaction management
- Migration support

### API Files

#### `src/api/server.rs`
- HTTP server using axum/warp
- Request routing
- Middleware stack
- CORS handling

#### `src/api/handlers.rs`
- API endpoint implementations
- Request validation
- Response formatting
- Error handling

#### `src/api/middleware.rs`
- Authentication middleware
- Rate limiting middleware
- Logging middleware
- Metrics collection

### Utility Files

#### `src/utils/logging.rs`
- Structured logging setup
- Multiple output formats
- Log level configuration
- Performance tracking

#### `src/utils/metrics.rs`
- Performance metrics collection
- Prometheus integration
- Custom metrics
- Alerting thresholds

#### `src/utils/validation.rs`
- Input validation functions
- Address validation
- Parameter sanitization
- Security checks

## Configuration Files

### `config/default.toml`
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

[cache]
ttl_seconds = 300
max_size = 10000

[turnkey]
api_url = "https://api.turnkey.com"
timeout_ms = 10000

[logging]
level = "info"
format = "json"
```

## Key Features Implementation

### 1. Multi-threaded Processing
- Use `rayon` for parallel wallet scanning
- Work-stealing thread pool
- Configurable concurrency levels
- CPU-bound task optimization

### 2. Connection Pooling
- Reusable RPC connections
- Health checking and failover
- Load balancing across endpoints
- Connection lifecycle management

### 3. Batch Processing
- Process wallets in configurable batches
- Aggregate RPC requests where possible
- Memory-efficient streaming
- Progress tracking

### 4. Rate Limiting
- Token bucket algorithm
- Per-endpoint rate limits
- Dynamic adjustment based on response
- Graceful degradation

### 5. Security
- Secure credential storage
- Input validation and sanitization
- Rate limiting to prevent abuse
- Audit logging

### 6. Monitoring
- Performance metrics collection
- Health check endpoints
- Error rate tracking
- Resource usage monitoring

## Deployment Considerations

### Docker Support
- Multi-stage builds
- Minimal runtime image
- Environment-specific configs
- Health checks

### Kubernetes
- Horizontal pod autoscaling
- Resource limits and requests
- ConfigMap and Secret management
- Service discovery

### Performance Tuning
- Connection pool sizing
- Thread pool configuration
- Cache optimization
- Database indexing

## Security Best Practices

1. **Credential Management**
   - Environment variables for secrets
   - Key rotation support
   - Encrypted storage

2. **API Security**
   - JWT authentication
   - Rate limiting
   - Input validation
   - CORS configuration

3. **Network Security**
   - TLS enforcement
   - Private RPC endpoints
   - VPN support
   - IP whitelisting

## Testing Strategy

### Unit Tests
- Core logic validation
- Error handling
- Edge cases
- Performance benchmarks

### Integration Tests
- End-to-end workflows
- API contract testing
- Database integration
- External service mocking

### Load Testing
- 1000+ concurrent wallets
- Memory usage profiling
- Response time validation
- Resource exhaustion testing

## Monitoring and Observability

### Metrics
- Request/response times
- Error rates
- Connection pool stats
- Cache hit ratios

### Logging
- Structured JSON logs
- Correlation IDs
- Security events
- Performance data

### Alerting
- High error rates
- Resource exhaustion
- Service downtime
- Security incidents

## Development Workflow

1. **Feature Development**
   - Feature branches
   - Code reviews
   - Automated testing
   - Documentation updates

2. **Release Process**
   - Semantic versioning
   - Change logs
   - Release notes
   - Deployment scripts

3. **Maintenance**
   - Dependency updates
   - Security patches
   - Performance optimization
   - Bug fixes
