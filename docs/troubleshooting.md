# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with Solana Recover. It covers installation problems, runtime errors, performance issues, and frequently asked questions.

## Table of Contents

- [Common Issues](#common-issues)
- [Installation Problems](#installation-problems)
- [Runtime Errors](#runtime-errors)
- [Performance Issues](#performance-issues)
- [Network and RPC Issues](#network-and-rpc-issues)
- [Database Issues](#database-issues)
- [API Issues](#api-issues)
- [FAQ](#faq)
- [Debug Mode](#debug-mode)
- [Getting Help](#getting-help)

## Common Issues

### Quick Diagnostics

Run this diagnostic script to check for common problems:

```bash
#!/bin/bash
echo "=== Solana Recover Diagnostics ==="

# Check Rust installation
if command -v rustc &> /dev/null; then
    echo "✓ Rust version: $(rustc --version)"
else
    echo "✗ Rust not found - please install Rust"
fi

# Check network connectivity
if curl -s https://api.mainnet-beta.solana.com > /dev/null; then
    echo "✓ Solana RPC endpoint reachable"
else
    echo "✗ Cannot reach Solana RPC endpoint"
fi

# Check available memory
MEMORY_MB=$(free -m | awk 'NR==2{printf "%.0f", $7}')
if [ $MEMORY_MB -gt 1000 ]; then
    echo "✓ Available memory: ${MEMORY_MB}MB"
else
    echo "⚠ Low memory: ${MEMORY_MB}MB (may affect performance)"
fi

# Check disk space
DISK_GB=$(df . | awk 'NR==2{printf "%.0f", $4/1024/1024}')
if [ $DISK_GB -gt 1 ]; then
    echo "✓ Available disk space: ${DISK_GB}GB"
else
    echo "⚠ Low disk space: ${DISK_GB}GB"
fi

echo "=== End Diagnostics ==="
```

## Installation Problems

### Rust Installation Issues

#### Problem: "cargo command not found"

**Solution:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
source ~/.cargo/env

# Verify installation
cargo --version
```

#### Problem: Build fails with "linker" errors

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# CentOS/RHEL
sudo yum groupinstall "Development Tools"

# macOS
xcode-select --install

# Windows
# Install Visual Studio Build Tools or Visual Studio Community
```

#### Problem: Out of memory during build

**Solution:**
```bash
# Limit parallel jobs
export CARGO_BUILD_JOBS=2

# Build with less optimization
cargo build --release --profile dev

# Or use pre-built binaries
wget https://github.com/Genius740Code/Sol-account-cleaner/releases/latest/download/solana-recover-linux-x64
chmod +x solana-recover-linux-x64
```

### Dependency Issues

#### Problem: "Failed to download crate"

**Solution:**
```bash
# Clear cargo cache
cargo clean

# Update registry
cargo update

# Try different registry mirror
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git
```

#### Problem: SSL/TLS errors during build

**Solution:**
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# macOS
brew install openssl

# Set SSL certificate path
export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
```

## Runtime Errors

### Configuration Errors

#### Problem: "Configuration file not found"

**Solution:**
```bash
# Create default config
mkdir -p config
cat > config/default.toml << EOF
[server]
host = "0.0.0.0"
port = 8080

[rpc]
endpoints = ["https://api.mainnet-beta.solana.com"]
pool_size = 10
timeout_ms = 5000

[scanner]
batch_size = 100
max_concurrent_wallets = 1000
EOF

# Or specify config file path
solana-recover server --config config/default.toml
```

#### Problem: "Invalid wallet address format"

**Solution:**
```bash
# Validate wallet address format
solana-keygen pubkey < wallet-file.txt

# Common format issues:
# - Extra whitespace
# - Missing characters
# - Invalid base58 encoding

# Example of valid address:
9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
```

### Permission Errors

#### Problem: "Permission denied" when accessing files

**Solution:**
```bash
# Check file permissions
ls -la config/
ls -la data/

# Fix permissions
chmod 755 config/
chmod 644 config/*.toml
chmod 755 data/

# Run as correct user
sudo -u solana solana-recover server
```

#### Problem: "Port already in use"

**Solution:**
```bash
# Find process using the port
netstat -tulpn | grep :8080
# or
lsof -i :8080

# Kill the process
kill -9 <PID>

# Or use different port
solana-recover server --port 8081
```

## Performance Issues

### Slow Scanning

#### Problem: Scans take too long

**Diagnosis:**
```bash
# Check RPC endpoint latency
curl -w "@curl-format.txt" -o /dev/null -s https://api.mainnet-beta.solana.com

# Monitor system resources
top
htop
iotop

# Check network bandwidth
iftop -i eth0
```

**Solutions:**
```toml
# config/production.toml
[rpc]
endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana"
]
pool_size = 50
timeout_ms = 3000

[scanner]
batch_size = 200
max_concurrent_wallets = 2000
```

```bash
# Optimize system settings
# Increase file descriptor limit
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Optimize network settings
echo "net.core.somaxconn = 65536" >> /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog = 65536" >> /etc/sysctl.conf
sysctl -p
```

### Memory Issues

#### Problem: Out of memory errors

**Diagnosis:**
```bash
# Monitor memory usage
free -h
watch -n 1 'free -h'

# Check process memory
ps aux --sort=-%mem | head -10

# Memory profiling
valgrind --tool=massif solana-recover scan WALLET_ADDRESS
```

**Solutions:**
```toml
# Reduce memory usage
[scanner]
batch_size = 50
max_concurrent_wallets = 500

[cache]
max_size = 1000
```

```bash
# Enable swap if needed
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

### High CPU Usage

#### Problem: CPU usage consistently high

**Diagnosis:**
```bash
# CPU profiling
perf record -g solana-recover server
perf report

# Check thread count
ps -eLf | grep solana-recover | wc -l

# Monitor CPU
top -p $(pgrep solana-recover)
```

**Solutions:**
```toml
# Optimize worker threads
[server]
workers = 4  # Match CPU cores

[scanner]
max_concurrent_wallets = 1000  # Reduce if CPU-bound
```

## Network and RPC Issues

### RPC Connection Problems

#### Problem: "Connection timeout" to Solana RPC

**Diagnosis:**
```bash
# Test RPC endpoint
curl -v https://api.mainnet-beta.solana.com

# Check DNS resolution
nslookup api.mainnet-beta.solana.com

# Test with different endpoint
curl -v https://solana-api.projectserum.com
```

**Solutions:**
```toml
# Configure multiple endpoints and timeouts
[rpc]
endpoints = [
  "https://api.mainnet-beta.solana.com",
  "https://solana-api.projectserum.com",
  "https://rpc.ankr.com/solana"
]
timeout_ms = 10000
retry_attempts = 3
health_check_interval_seconds = 30
```

#### Problem: "Rate limit exceeded" from RPC

**Solutions:**
```toml
# Reduce rate limiting
[rpc]
rate_limit_rps = 50
pool_size = 20

[scanner]
batch_size = 50
max_concurrent_wallets = 500
```

```bash
# Use dedicated RPC provider
# Sign up for services like:
# - QuickNode
# - Alchemy
# - Helius
# - Triton

# Example with QuickNode
export SOLANA_RPC_ENDPOINTS="https://YOUR-QUICKNODE-ENDPOINT.solana-mainnet.quiknode.pro"
```

### Network Connectivity

#### Problem: "No route to host"

**Diagnosis:**
```bash
# Check network connectivity
ping 8.8.8.8
traceroute api.mainnet-beta.solana.com

# Check firewall rules
sudo ufw status
iptables -L

# Check proxy settings
echo $http_proxy
echo $https_proxy
```

**Solutions:**
```bash
# Configure proxy if needed
export http_proxy=http://proxy.company.com:8080
export https_proxy=http://proxy.company.com:8080

# Open firewall ports
sudo ufw allow 8080
sudo ufw allow 9090
```

## Database Issues

### PostgreSQL Problems

#### Problem: "Connection refused" to database

**Diagnosis:**
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Test connection
psql -h localhost -U postgres -d solana_recover

# Check logs
sudo tail -f /var/log/postgresql/postgresql-*.log
```

**Solutions:**
```bash
# Start PostgreSQL
sudo systemctl start postgresql
sudo systemctl enable postgresql

# Check configuration
sudo -u postgres psql -c "SHOW listen_addresses;"

# Create database if needed
sudo -u postgres createdb solana_recover
```

#### Problem: Database connection pool exhausted

**Diagnosis:**
```bash
# Check active connections
sudo -u postgres psql -c "SELECT count(*) FROM pg_stat_activity;"

# Monitor connection usage
watch -n 1 'sudo -u postgres psql -c "SELECT state, count(*) FROM pg_stat_activity GROUP BY state;"'
```

**Solutions:**
```toml
# Increase pool size
[database]
pool_size = 50
timeout_seconds = 30

# Or reduce concurrent operations
[scanner]
max_concurrent_wallets = 500
```

### Migration Issues

#### Problem: Database migration fails

**Diagnosis:**
```bash
# Check migration status
solana-recover migrate --status

# Run with debug logging
RUST_LOG=debug solana-recover migrate

# Check database schema
sudo -u postgres psql -d solana_recover -c "\dt"
```

**Solutions:**
```bash
# Reset database (WARNING: This deletes all data)
sudo -u postgres dropdb solana_recover
sudo -u postgres createdb solana_recover
solana-recover migrate

# Or manually fix migration
sudo -u postgres psql -d solana_recover
# Fix schema manually
```

## API Issues

### Authentication Problems

#### Problem: "Invalid API key" error

**Diagnosis:**
```bash
# Test API key
curl -H "X-API-Key: your-api-key" \
     -H "Content-Type: application/json" \
     https://api.solana-recover.com/health

# Check key format
echo "your-api-key" | wc -c
# Should be 32+ characters
```

**Solutions:**
```bash
# Generate new API key
# Visit: https://dashboard.solana-recover.com/api-keys

# Store securely
export SOLANA_RECOVER_API_KEY=$(cat ~/.solana-recover/api-key)

# Use in requests
curl -H "X-API-Key: $SOLANA_RECOVER_API_KEY" \
     https://api.solana-recover.com/api/v1/scan
```

#### Problem: JWT token expired

**Diagnosis:**
```bash
# Decode JWT token
echo "your-jwt-token" | cut -d. -f2 | base64 -d | jq .

# Check expiration
echo "your-jwt-token" | cut -d. -f2 | base64 -d | jq .exp
```

**Solutions:**
```bash
# Refresh token
curl -X POST https://api.solana-recover.com/auth/refresh \
  -H "Authorization: Bearer old-token"

# Or login again
curl -X POST https://api.solana-recover.com/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com", "password": "password"}'
```

### Rate Limiting

#### Problem: "429 Too Many Requests"

**Diagnosis:**
```bash
# Check rate limit headers
curl -I https://api.solana-recover.com/api/v1/scan \
  -H "X-API-Key: your-api-key"

# Headers to check:
# X-RateLimit-Limit: 1000
# X-RateLimit-Remaining: 850
# X-RateLimit-Reset: 1642248000
```

**Solutions:**
```bash
# Implement exponential backoff
function api_call() {
  local retry=0
  while [ $retry -lt 3 ]; do
    response=$(curl -s -w "%{http_code}" -o response.json \
      -H "X-API-Key: $API_KEY" \
      "$1")
    
    if [ "$response" = "429" ]; then
      sleep $((2 ** retry))
      retry=$((retry + 1))
    else
      break
    fi
  done
}

# Upgrade plan for higher limits
# Visit: https://dashboard.solana-recover.com/billing
```

## FAQ

### General Questions

**Q: What is Solana Recover?**
A: Solana Recover is a high-performance tool for finding and recovering rent deposits from unused Solana token accounts.

**Q: How much SOL can I recover?**
A: It varies by wallet. Empty token accounts typically hold 0.00203928 SOL each in rent deposits.

**Q: Is it safe to use?**
A: Yes. The tool only reads wallet data and never requests private keys or initiates transactions.

**Q: What does it cost?**
A: The tool is open source and free. API services have usage-based pricing.

### Technical Questions

**Q: Why does scanning take time?**
A: Each wallet scan requires querying multiple RPC endpoints to check all token accounts. Network latency and RPC rate limits affect speed.

**Q: Can I scan multiple wallets at once?**
A: Yes. Use the batch scan feature to process multiple wallets concurrently.

**Q: What wallet addresses are supported?**
A: Any valid Solana public key (base58-encoded) is supported.

**Q: How accurate are the results?**
A: Results are highly accurate as they use direct RPC queries to the Solana blockchain.

### Business Questions

**Q: Can I use this commercially?**
A: Yes. The project is MIT licensed and can be used in commercial applications.

**Q: Do you offer enterprise support?**
A: Yes. Contact enterprise@solana-recover.com for enterprise plans and support.

**Q: Can I host my own instance?**
A: Yes. The software can be self-hosted. See the deployment guide for details.

## Debug Mode

### Enable Debug Logging

```bash
# Environment variable
export RUST_LOG=debug

# Command line
solana-recover --log-level debug scan WALLET_ADDRESS

# Configuration file
[logging]
level = "debug"
format = "pretty"
```

### Debug Configuration

```toml
# config/debug.toml
[logging]
level = "trace"
format = "json"
output = "file"
file_path = "/var/log/solana-recover/debug.log"

[debug]
enable_sql_logging = true
enable_request_logging = true
enable_performance_profiling = true
```

### Common Debug Commands

```bash
# Scan with full debugging
RUST_LOG=trace solana-recover scan WALLET_ADDRESS

# Server with debug mode
RUST_LOG=debug solana-recover server --config config/debug.toml

# Test RPC connectivity
RUST_LOG=debug solana-recover test-rpc

# Validate configuration
solana-recover config validate

# Check database connection
solana-recover test-db
```

### Performance Profiling

```bash
# CPU profiling
perf record -g solana-recover scan WALLET_ADDRESS
perf report

# Memory profiling
valgrind --tool=massif solana-recover scan WALLET_ADDRESS
ms_print massif.out.*

# Flame graph
cargo install flamegraph
flamegraph --bin solana-recover -- scan WALLET_ADDRESS
```

## Getting Help

### Self-Service Resources

1. **Documentation**: [docs.solana-recover.com](https://docs.solana-recover.com)
2. **Examples**: [GitHub Examples](../examples/)
3. **FAQ**: See above section
4. **Troubleshooting**: This guide

### Community Support

- **Discord**: [Join our community](https://discord.gg/solana-recover)
- **GitHub Discussions**: [Start a discussion](https://github.com/Genius740Code/Sol-account-cleaner/discussions)
- **Stack Overflow**: Tag questions with `solana-recover`

### Professional Support

- **Email**: support@solana-recover.com
- **Enterprise**: enterprise@solana-recover.com
- **Security**: security@solana-recover.com

### Bug Reports

When reporting bugs, include:

1. **Version**: `solana-recover --version`
2. **Environment**: OS, Rust version, memory, CPU
3. **Configuration**: Relevant config settings
4. **Logs**: Full error logs with debug mode enabled
5. **Steps to Reproduce**: Clear reproduction steps
6. **Expected vs Actual**: What you expected vs what happened

```bash
# Bug report template
solana-recover --version
rustc --version
uname -a
free -h
df -h

RUST_LOG=debug solana-recover scan YOUR_WALLET_ADDRESS 2>&1 | tee bug-report.log
```

### Feature Requests

For feature requests:

1. Check existing issues first
2. Provide clear use case
3. Explain expected benefits
4. Consider implementation complexity
5. Offer to contribute if possible

---

If you're still having issues after trying these solutions, don't hesitate to reach out. Our team is here to help you succeed with Solana Recover.
