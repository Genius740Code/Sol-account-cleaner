# Configuration Reference

This comprehensive reference covers all configuration options available in Solana Recover, including server settings, database configuration, RPC endpoints, performance tuning, and security settings.

## Table of Contents

- [Configuration Overview](#configuration-overview)
- [Configuration Files](#configuration-files)
- [Environment Variables](#environment-variables)
- [Server Configuration](#server-configuration)
- [Database Configuration](#database-configuration)
- [Redis Configuration](#redis-configuration)
- [RPC Configuration](#rpc-configuration)
- [Scanner Configuration](#scanner-configuration)
- [Fee Configuration](#fee-configuration)
- [Security Configuration](#security-configuration)
- [Logging Configuration](#logging-configuration)
- [Monitoring Configuration](#monitoring-configuration)
- [Cache Configuration](#cache-configuration)
- [Performance Tuning](#performance-tuning)
- [Configuration Validation](#configuration-validation)

## Configuration Overview

### Configuration Priority

Settings are applied in the following order (highest to lowest priority):

1. **Command Line Arguments** - Direct CLI parameters
2. **Environment Variables** - System environment variables
3. **Configuration Files** - TOML configuration files
4. **Default Values** - Built-in defaults

### Configuration Sources

```bash
# Command line
solana-recover server --port 8080 --workers 8

# Environment variables
export SOLANA_RECOVER_PORT=8080
export SOLANA_RECOVER_WORKERS=8

# Configuration file
solana-recover server --config config/production.toml
```

## Configuration Files

### File Locations

The application searches for configuration files in this order:

1. `--config` command line argument
2. `SOLANA_RECOVER_CONFIG` environment variable
3. `./config/default.toml`
4. `./config.toml`
5. `$HOME/.solana-recover/config.toml`
6. `/etc/solana-recover/config.toml`

### File Structure

```
config/
├── default.toml          # Default configuration
├── development.toml      # Development overrides
├── production.toml       # Production settings
├── testing.toml          # Test environment
└── local.toml           # Local development
```

### Complete Configuration Example

```toml
# config/production.toml

[server]
host = "0.0.0.0"
port = 8080
workers = 8
timeout_seconds = 60
max_connections = 10000
keep_alive_seconds = 30

[database]
url = "${DATABASE_URL}"
pool_size = 20
timeout_seconds = 30
statement_timeout_seconds = 30
migration_auto = true
max_connections = 100

[redis]
url = "${REDIS_URL}"
pool_size = 10
timeout_seconds = 5
max_connections = 50

[rpc]
endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana"
]
pool_size = 50
timeout_ms = 5000
rate_limit_rps = 100
health_check_interval_seconds = 30
retry_attempts = 3
retry_delay_ms = 1000
load_balancing = "round_robin"

[scanner]
batch_size = 100
max_concurrent_wallets = 1000
queue_size = 10000
retry_attempts = 3
retry_delay_ms = 1000
scan_timeout_seconds = 60
max_accounts_per_wallet = 10000

[fees]
default_percentage = 0.15
minimum_lamports = 1000000
maximum_lamports = 100000000
waive_below_lamports = 10000000
enterprise_percentage = 0.10
volume_discount_tiers = [
  { volume_sol = 100.0, discount = 0.05 },
  { volume_sol = 1000.0, discount = 0.10 },
  { volume_sol = 10000.0, discount = 0.15 }
]

[security]
jwt_secret = "${JWT_SECRET}"
jwt_expiry_hours = 24
api_key_encryption_key = "${API_KEY_ENCRYPTION_KEY}"
cors_origins = ["https://yourdomain.com"]
cors_methods = ["GET", "POST", "PUT", "DELETE"]
cors_headers = ["Content-Type", "Authorization"]
rate_limiting_enabled = true
rate_limit_requests_per_minute = 100
ssl_required = true
trusted_proxies = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]

[logging]
level = "info"
format = "json"
output = "file"
file_path = "/var/log/solana-recover/app.log"
max_file_size_mb = 100
max_files = 10
json_fields = ["timestamp", "level", "message", "request_id", "user_id"]
console_colors = false

[monitoring]
metrics_enabled = true
metrics_port = 9090
metrics_path = "/metrics"
health_check_interval = 30
prometheus_enabled = true
jaeger_enabled = false
jaeger_endpoint = "http://localhost:14268/api/traces"

[cache]
ttl_seconds = 300
max_size = 10000
cleanup_interval_seconds = 60
redis_cache_enabled = true
cache_wallet_results = true
cache_rpc_responses = true

[performance]
tokio_threads = "auto"
max_blocking_threads = 512
stack_size = 2097152
buffer_size = 8192
enable_zero_copy = true
memory_allocator = "jemalloc"

[features]
api_server_enabled = true
cli_enabled = true
batch_processing_enabled = true
webhooks_enabled = true
experimental_features = false
```

## Environment Variables

### Application Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `SOLANA_RECOVER_ENV` | Environment (development/production) | `development` | `production` |
| `SOLANA_RECOVER_CONFIG` | Configuration file path | `config/default.toml` | `/etc/solana-recover/config.toml` |
| `SOLANA_RECOVER_LOG_LEVEL` | Logging level | `info` | `debug` |
| `SOLANA_RECOVER_HOST` | Server bind address | `0.0.0.0` | `127.0.0.1` |
| `SOLANA_RECOVER_PORT` | Server port | `8080` | `9000` |
| `SOLANA_RECOVER_WORKERS` | Number of worker threads | `4` | `8` |

### Database Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `DATABASE_URL` | PostgreSQL connection string | - | `postgresql://user:pass@localhost:5432/db` |
| `DATABASE_POOL_SIZE` | Database connection pool size | `20` | `50` |
| `DATABASE_TIMEOUT_SECONDS` | Database timeout | `30` | `60` |

### Redis Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `REDIS_URL` | Redis connection string | - | `redis://localhost:6379` |
| `REDIS_POOL_SIZE` | Redis connection pool size | `10` | `20` |
| `REDIS_TIMEOUT_SECONDS` | Redis timeout | `5` | `10` |

### RPC Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `SOLANA_RPC_ENDPOINTS` | Comma-separated RPC endpoints | `https://api.mainnet-beta.solana.com` | `https://api.mainnet-beta.solana.com,https://solana-api.projectserum.com` |
| `SOLANA_RPC_POOL_SIZE` | RPC connection pool size | `50` | `100` |
| `SOLANA_RPC_TIMEOUT_MS` | RPC timeout in milliseconds | `5000` | `10000` |
| `SOLANA_RPC_RATE_LIMIT_RPS` | RPC rate limit (requests/second) | `100` | `200` |

### Security Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `JWT_SECRET` | JWT signing secret | - | `your-super-secret-jwt-key` |
| `API_KEY_ENCRYPTION_KEY` | API key encryption key | - | `your-32-character-key` |
| `CORS_ORIGINS` | Comma-separated CORS origins | `*` | `https://yourdomain.com,https://app.yourdomain.com` |

## Server Configuration

### Basic Settings

```toml
[server]
host = "0.0.0.0"              # Bind address
port = 8080                   # Port number
workers = 8                   # Number of worker threads
timeout_seconds = 60          # Request timeout
max_connections = 10000       # Maximum concurrent connections
keep_alive_seconds = 30       # Keep-alive timeout
```

### Advanced Settings

```toml
[server]
# TLS/SSL configuration
tls_enabled = true
tls_cert_path = "/etc/ssl/certs/solana-recover.crt"
tls_key_path = "/etc/ssl/private/solana-recover.key"

# Request limits
max_request_size_mb = 10
max_header_size_kb = 8
max_body_size_mb = 100

# Connection limits
max_concurrent_streams = 1000
initial_window_size = 65535
max_frame_size = 16777215
```

### Performance Settings

```toml
[server]
# Thread pool configuration
tokio_threads = "auto"        # "auto" or number
max_blocking_threads = 512    # Maximum blocking threads
stack_size = 2097152          # Thread stack size in bytes

# Buffer configuration
buffer_size = 8192            # Buffer size for I/O operations
enable_zero_copy = true       # Enable zero-copy optimizations
```

## Database Configuration

### PostgreSQL Settings

```toml
[database]
url = "postgresql://user:password@localhost:5432/solana_recover"
pool_size = 20                # Connection pool size
timeout_seconds = 30         # Connection timeout
statement_timeout_seconds = 30  # Statement timeout
migration_auto = true        # Auto-run migrations
max_connections = 100         # Maximum connections

# SSL configuration
ssl_mode = "prefer"           # disable, prefer, require
ssl_cert = "/path/to/cert"
ssl_key = "/path/to/key"
ssl_root_cert = "/path/to/root_cert"

# Connection pool settings
min_idle = 5                 # Minimum idle connections
max_lifetime_seconds = 1800   # Connection lifetime
idle_timeout_seconds = 600    # Idle timeout
```

### Advanced Database Settings

```toml
[database]
# Performance tuning
shared_preload_libraries = "pg_stat_statements"
effective_cache_size = "1GB"
work_mem = "4MB"
maintenance_work_mem = "64MB"
checkpoint_completion_target = 0.9
wal_buffers = "16MB"
default_statistics_target = 100

# Logging
log_min_duration_statement = 1000
log_checkpoints = true
log_connections = true
log_disconnections = true
log_lock_waits = true
```

### Database Migration

```toml
[database.migration]
auto = true                   # Auto-run migrations
directory = "migrations"      # Migration directory
table = "_schema_migrations"  # Migration tracking table
lock_timeout_seconds = 60     # Migration lock timeout
```

## Redis Configuration

### Basic Settings

```toml
[redis]
url = "redis://localhost:6379"
pool_size = 10                # Connection pool size
timeout_seconds = 5           # Command timeout
max_connections = 50           # Maximum connections

# Authentication
password = "redis-password"   # Redis password
database = 0                  # Database number
```

### Advanced Redis Settings

```toml
[redis]
# Connection settings
connection_name = "solana-recover"
keep_alive_seconds = 30
command_timeout_seconds = 5
reconnect_attempts = 3
reconnect_delay_ms = 1000

# Cluster configuration
cluster_nodes = [
  "redis://node1:6379",
  "redis://node2:6379",
  "redis://node3:6379"
]
cluster_password = "cluster-password"

# Sentinel configuration
sentinel_nodes = [
  "redis://sentinel1:26379",
  "redis://sentinel2:26379"
]
sentinel_service_name = "mymaster"
```

## RPC Configuration

### Solana RPC Endpoints

```toml
[rpc]
endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana",
  "https://solana-mainnet.rpc.extrnode.com"
]
pool_size = 50                # Connection pool size
timeout_ms = 5000            # Request timeout
rate_limit_rps = 100          # Rate limit per endpoint
```

### Load Balancing

```toml
[rpc]
load_balancing = "round_robin"  # round_robin, random, least_connections
health_check_interval_seconds = 30
retry_attempts = 3
retry_delay_ms = 1000
circuit_breaker_enabled = true
circuit_breaker_threshold = 5
circuit_breaker_timeout_seconds = 60
```

### Advanced RPC Settings

```toml
[rpc]
# Request configuration
commitment = "confirmed"        # processed, confirmed, finalized
encoding = "base64"            # base64, base58, json
max_supported_transaction_version = 0

# Performance tuning
enable_ws = true               # Enable WebSocket connections
ws_pool_size = 10
ws_timeout_seconds = 30
subscription_timeout_seconds = 300

# Rate limiting
rate_limit_strategy = "token_bucket"
rate_limit_bucket_size = 100
rate_limit_refill_rate = 10
```

## Scanner Configuration

### Basic Scanner Settings

```toml
[scanner]
batch_size = 100               # Wallets per batch
max_concurrent_wallets = 1000  # Maximum concurrent scans
queue_size = 10000            # Scan queue size
retry_attempts = 3             # Retry attempts
retry_delay_ms = 1000         # Retry delay
scan_timeout_seconds = 60      # Scan timeout per wallet
```

### Advanced Scanner Settings

```toml
[scanner]
# Account limits
max_accounts_per_wallet = 10000
max_empty_accounts = 1000
min_balance_lamports = 0

# Performance tuning
enable_parallel_account_fetch = true
account_fetch_batch_size = 100
account_fetch_timeout_ms = 2000

# Caching
cache_wallet_results = true
cache_empty_accounts = true
cache_duration_seconds = 300

# Progress tracking
enable_progress_reporting = true
progress_report_interval_seconds = 10
```

### Scanner Limits

```toml
[scanner.limits]
# Rate limiting per user
max_scans_per_minute = 60
max_batch_size_per_request = 1000
max_concurrent_scans_per_user = 10

# Resource limits
max_memory_per_scan_mb = 100
max_cpu_time_per_scan_seconds = 30
max_network_requests_per_scan = 1000
```

## Fee Configuration

### Basic Fee Structure

```toml
[fees]
default_percentage = 0.15      # 15% default fee
minimum_lamports = 1000000     # Minimum fee in lamports
maximum_lamports = 100000000  # Maximum fee in lamports
waive_below_lamports = 10000000  # Waive fees below this amount
```

### Advanced Fee Structure

```toml
[fees]
# Enterprise pricing
enterprise_percentage = 0.10   # 10% for enterprise users
enterprise_minimum_lamports = 5000000

# Volume discounts
volume_discount_tiers = [
  { volume_sol = 100.0, discount = 0.05 },   # 5% discount for 100+ SOL
  { volume_sol = 1000.0, discount = 0.10 },  # 10% discount for 1000+ SOL
  { volume_sol = 10000.0, discount = 0.15 }   # 15% discount for 10000+ SOL
]

# Time-based pricing
peak_hours_discount = 0.02    # 2% discount during peak hours
off_peak_discount = 0.05      # 5% discount during off-peak hours

# Special pricing
vip_discount = 0.20           # 20% discount for VIP users
partner_discount = 0.15      # 15% discount for partners
```

### Fee Calculation Rules

```toml
[fees.calculation]
# Rounding rules
round_to_nearest_lamport = true
round_up_threshold = 500000   # Round up if above this amount

# Fee caps
max_fee_percentage_of_recovery = 0.50  # Max 50% of recovered amount
max_daily_fee_per_user_lamports = 1000000000  # Max daily fee per user

# Exemptions
exempt_wallets = [
  "WALLET_ADDRESS_1",
  "WALLET_ADDRESS_2"
]
exempt_minimum_balance_lamports = 10000000000
```

## Security Configuration

### Authentication

```toml
[security]
jwt_secret = "${JWT_SECRET}"
jwt_expiry_hours = 24
jwt_refresh_enabled = true
jwt_refresh_hours = 168  # 7 days

# API key management
api_key_encryption_key = "${API_KEY_ENCRYPTION_KEY}"
api_key_length = 32
api_key_expiry_days = 365
```

### CORS Configuration

```toml
[security.cors]
origins = ["https://yourdomain.com", "https://app.yourdomain.com"]
methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
headers = ["Content-Type", "Authorization", "X-API-Key"]
max_age_seconds = 86400
allow_credentials = true
```

### Rate Limiting

```toml
[security.rate_limiting]
enabled = true
requests_per_minute = 100
burst_size = 20
whitelist_ips = ["127.0.0.1", "10.0.0.0/8"]
blacklist_ips = ["192.168.1.100"]

# Advanced rate limiting
rate_limit_strategy = "sliding_window"
window_size_seconds = 60
cleanup_interval_seconds = 300
```

### SSL/TLS Configuration

```toml
[security.ssl]
enabled = true
cert_path = "/etc/ssl/certs/solana-recover.crt"
key_path = "/etc/ssl/private/solana-recover.key"
ca_path = "/etc/ssl/certs/ca-certificates.crt"

# SSL protocols and ciphers
protocols = ["TLSv1.2", "TLSv1.3"]
ciphers = [
  "ECDHE-RSA-AES256-GCM-SHA384",
  "ECDHE-RSA-AES128-GCM-SHA256",
  "ECDHE-RSA-AES256-SHA384"
]
prefer_server_ciphers = true
```

## Logging Configuration

### Basic Logging

```toml
[logging]
level = "info"               # trace, debug, info, warn, error
format = "json"              # json, pretty, compact
output = "file"               # stdout, stderr, file
file_path = "/var/log/solana-recover/app.log"
```

### Advanced Logging

```toml
[logging]
# File rotation
max_file_size_mb = 100
max_files = 10
compress_files = true
archive_directory = "/var/log/solana-recover/archive"

# Log filtering
modules = [
  "solana_recover=info",
  "solana_recover::scanner=debug",
  "solana_recover::rpc=warn"
]
exclude_modules = ["hyper=warn", "tokio=warn"]

# JSON logging
json_fields = [
  "timestamp", "level", "message", "request_id",
  "user_id", "wallet_address", "duration_ms"
]
json_pretty_print = false
```

### Structured Logging

```toml
[logging.structured]
# Request logging
log_requests = true
log_request_body = false
log_response_body = false
log_headers = false

# Performance logging
log_slow_queries = true
slow_query_threshold_ms = 1000
log_memory_usage = true
memory_log_interval_seconds = 60

# Error logging
log_stack_traces = true
log_panic_messages = true
log_to_sentry = false
sentry_dsn = "${SENTRY_DSN}"
```

## Monitoring Configuration

### Metrics Configuration

```toml
[monitoring]
metrics_enabled = true
metrics_port = 9090
metrics_path = "/metrics"
health_check_interval = 30

# Prometheus configuration
prometheus_enabled = true
prometheus_namespace = "solana_recover"
prometheus_subsystem = "api"

# Custom metrics
custom_metrics_enabled = true
metrics_retention_days = 30
metrics_aggregation_interval_seconds = 10
```

### Tracing Configuration

```toml
[monitoring.tracing]
jaeger_enabled = false
jaeger_endpoint = "http://localhost:14268/api/traces"
jaeger_service_name = "solana-recover"
jaeger_sample_rate = 0.1

# OpenTelemetry
otel_enabled = false
otel_endpoint = "http://localhost:4317"
otel_service_name = "solana-recover"
otel_headers = ["x-trace-id", "x-span-id"]
```

### Health Checks

```toml
[monitoring.health_checks]
database_check_enabled = true
database_check_interval_seconds = 30
database_check_timeout_seconds = 5

redis_check_enabled = true
redis_check_interval_seconds = 30
redis_check_timeout_seconds = 5

rpc_check_enabled = true
rpc_check_interval_seconds = 60
rpc_check_timeout_seconds = 10

disk_space_check_enabled = true
disk_space_threshold_percent = 90
memory_usage_check_enabled = true
memory_usage_threshold_percent = 90
```

## Cache Configuration

### Basic Cache Settings

```toml
[cache]
ttl_seconds = 300            # Cache TTL in seconds
max_size = 10000             # Maximum cache entries
cleanup_interval_seconds = 60  # Cleanup interval
```

### Advanced Cache Settings

```toml
[cache]
# Redis cache
redis_cache_enabled = true
redis_cache_prefix = "solana_recover:"
redis_cache_ttl_seconds = 3600

# Cache strategies
cache_wallet_results = true
cache_rpc_responses = true
cache_fee_calculations = true
cache_user_sessions = true

# Cache invalidation
invalidate_on_wallet_update = true
invalidate_on_fee_change = true
invalidate_on_user_logout = true
```

### Cache Performance

```toml
[cache.performance]
# Memory cache
memory_cache_enabled = true
memory_cache_size_mb = 100
memory_cache_eviction_policy = "lru"  # lru, lfu, fifo

# Compression
compress_cache_entries = true
compression_threshold_bytes = 1024
compression_algorithm = "gzip"  # gzip, lz4, snappy

# Serialization
serialization_format = "bincode"  # json, bincode, messagepack
```

## Performance Tuning

### Thread Pool Configuration

```toml
[performance]
tokio_threads = "auto"        # Number of Tokio threads
max_blocking_threads = 512    # Maximum blocking threads
stack_size = 2097152          # Thread stack size (2MB)

# CPU affinity
cpu_affinity_enabled = false
cpu_affinity_cores = [0, 1, 2, 3]
```

### Memory Configuration

```toml
[performance.memory]
# Memory allocator
allocator = "jemalloc"         # system, jemalloc, mimalloc
allocator_background_threads = 8

# Memory limits
max_heap_size_mb = 4096
max_stack_size_mb = 64
gc_enabled = true
gc_interval_seconds = 300
```

### I/O Configuration

```toml
[performance.io]
# Buffer sizes
read_buffer_size = 8192
write_buffer_size = 8192
network_buffer_size = 65536

# Async I/O
io_uring_enabled = false       # Linux only
aio_enabled = true
aio_max_requests = 1000
```

## Configuration Validation

### Built-in Validation

```bash
# Validate configuration
solana-recover config validate

# Validate specific file
solana-recover config validate --config production.toml

# Show configuration
solana-recover config show

# Test configuration
solana-recover config test
```

### Custom Validation Rules

```toml
[validation]
# Required fields
required_fields = [
  "server.host",
  "server.port",
  "database.url"
]

# Field validation
field_validation = [
  { field = "server.port", type = "port" },
  { field = "database.pool_size", type = "positive_integer" },
  { field = "rpc.endpoints", type = "url_array" }
]

# Cross-field validation
cross_field_validation = [
  { 
    rule = "scanner.max_concurrent_wallets <= database.pool_size * 10",
    message = "Scanner concurrency too high for database pool size"
  }
]
```

### Configuration Templates

```bash
# Generate default configuration
solana-recover config generate --template default > config/default.toml

# Generate production template
solana-recover config generate --template production > config/production.toml

# Generate with custom values
solana-recover config generate \
  --template production \
  --set server.port=9000 \
  --set database.pool_size=50 \
  > config/custom.toml
```

---

This configuration reference provides comprehensive documentation for all available settings. For specific use cases and examples, see the [Getting Started Guide](getting-started.md) and [Deployment Guide](deployment.md).
