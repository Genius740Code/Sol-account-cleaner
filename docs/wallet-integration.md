# Wallet Integration Guide

This guide covers integrating various wallet providers with Solana Recover, including Turnkey, Phantom, Solflare, and custom wallet implementations.

## Table of Contents

- [Supported Wallet Providers](#supported-wallet-providers)
- [Turnkey Integration](#turnkey-integration)
- [Phantom Integration](#phantom-integration)
- [Solflare Integration](#solflare-integration)
- [Custom Wallet Providers](#custom-wallet-providers)
- [Security Best Practices](#security-best-practices)
- [Troubleshooting](#troubleshooting)

## Supported Wallet Providers

### Overview

Solana Recover supports multiple wallet providers out of the box:

| Provider | Type | Authentication | Signing | Enterprise Ready |
|----------|-------|----------------|--------|------------------|
| Turnkey | API-based | ✅ | ✅ |
| Phantom | Browser Extension | ✅ | ❌ |
| Solflare | Browser Extension | ✅ | ❌ |
| Custom | SDK | ✅ | ✅ |

### Provider Comparison

#### Turnkey (Recommended for Enterprise)
- **Pros**: Enterprise-grade, MPC security, audit trails, API-based
- **Cons**: Requires setup, subscription costs
- **Best for**: Enterprise applications, high-security requirements

#### Phantom (Recommended for Consumers)
- **Pros**: User-friendly, browser-based, free
- **Cons**: Limited enterprise features, browser-dependent
- **Best for**: Consumer applications, web-based wallets

#### Solflare (Alternative to Phantom)
- **Pros**: Similar to Phantom, good mobile support
- **Cons**: Smaller user base
- **Best for**: Mobile applications, Phantom alternative

## Turnkey Integration

### Prerequisites

1. **Turnkey Account**
   - Sign up at [https://app.turnkey.com](https://app.turnkey.com)
   - Create organization
   - Generate API credentials

2. **Required Credentials**
   - Organization ID
   - API Key
   - Private Key ID

### Configuration

#### Environment Variables
```bash
export TURNKEY_API_URL="https://api.turnkey.com"
export TURNKEY_ORG_ID="your-organization-id"
export TURNKEY_API_KEY="your-api-key"
export TURNKEY_PRIVATE_KEY_ID="your-private-key-id"
```

#### Configuration File
```toml
[turnkey]
api_url = "https://api.turnkey.com"
organization_id = "${TURNKEY_ORG_ID}"
api_key = "${TURNKEY_API_KEY}"
private_key_id = "${TURNKEY_PRIVATE_KEY_ID}"
timeout_seconds = 30
retry_attempts = 3
```

### API Integration

#### Connect Wallet
```bash
curl -X POST http://localhost:8080/api/v1/wallets/connect \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_type": "turnkey",
    "credentials": {
      "api_key": "your-turnkey-api-key",
      "organization_id": "your-org-id",
      "private_key_id": "your-key-id"
    }
  }'
```

**Response:**
```json
{
  "connection_id": "conn-123456",
  "wallet_type": "turnkey",
  "public_key": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
  "created_at": "2024-01-15T10:30:00Z",
  "expires_at": "2024-01-16T10:30:00Z"
}
```

#### Sign Transaction
```bash
curl -X POST http://localhost:8080/api/v1/recover \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{
    "wallet_address": "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM",
    "empty_accounts": ["account1", "account2"],
    "destination_address": "dest-wallet",
    "wallet_connection_id": "conn-123456"
  }'
```

### SDK Integration

#### JavaScript/TypeScript
```javascript
import { TurnkeyProvider } from '@solana-recover/wallets';

const turnkeyProvider = new TurnkeyProvider({
  apiUrl: 'https://api.turnkey.com',
  organizationId: 'your-org-id',
  apiKey: 'your-api-key'
});

// Connect wallet
const connection = await turnkeyProvider.connect({
  privateKeyId: 'your-key-id'
});

// Sign transaction
const signature = await turnkeyProvider.signTransaction(
  connection,
  transactionBytes
);
```

#### Python
```python
from solana_recover.wallets import TurnkeyProvider

provider = TurnkeyProvider(
    api_url="https://api.turnkey.com",
    organization_id="your-org-id",
    api_key="your-api-key"
)

# Connect wallet
connection = await provider.connect({
    "private_key_id": "your-key-id"
})

# Sign transaction
signature = await provider.sign_transaction(
    connection,
    transaction_bytes
)
```

#### Rust
```rust
use solana_recover::wallet::TurnkeyProvider;

let provider = TurnkeyProvider::new();

// Connect wallet
let credentials = WalletCredentials {
    credentials: WalletCredentialData::Turnkey {
        api_key: "your-api-key".to_string(),
        organization_id: "your-org-id".to_string(),
        private_key_id: "your-key-id".to_string(),
    }
};

let connection = provider.connect(&credentials).await?;

// Sign transaction
let signature = provider.sign_transaction(&connection, &transaction_bytes).await?;
```

## Phantom Integration

### Prerequisites

1. **Phantom Extension**
   - Install Phantom browser extension
   - Create or import wallet

2. **Web Application**
   - Serve over HTTPS (required for wallet communication)
   - Include Phantom connection library

### Integration

#### HTML/JavaScript
```html
<!DOCTYPE html>
<html>
<head>
    <title>Solana Recover - Phantom Integration</title>
    <script src="https://unpkg.com/@solana/web3.js@latest/lib/index.iife.min.js"></script>
</head>
<body>
    <button id="connectPhantom">Connect Phantom</button>
    <button id="scanWallet">Scan Wallet</button>
    
    <script>
        const { Connection, PublicKey } = solanaWeb3;
        
        // Connect to Phantom
        document.getElementById('connectPhantom').onclick = async () => {
            try {
                const response = await window.solana.connect();
                console.log('Connected with public key:', response.publicKey.toString());
                
                // Send to backend
                await fetch('/api/v1/wallets/connect', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        wallet_type: 'phantom',
                        public_key: response.publicKey.toString()
                    })
                });
            } catch (error) {
                console.error('Failed to connect Phantom:', error);
            }
        };
        
        // Scan wallet
        document.getElementById('scanWallet').onclick = async () => {
            if (window.solana.isConnected) {
                const response = await fetch('/api/v1/scan', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        wallet_address: window.solana.publicKey.toString()
                    })
                });
                
                const result = await response.json();
                console.log('Scan result:', result);
            }
        };
    </script>
</body>
</html>
```

#### Backend Integration
```rust
// Add Phantom wallet type to your configuration
[wallet.phantom]
enabled = true
timeout_seconds = 30
```

## Solflare Integration

### Prerequisites

1. **Solflare Extension**
   - Install Solflare browser extension
   - Create or import wallet

2. **Web Application**
   - Similar requirements to Phantom

### Integration

#### JavaScript Integration
```javascript
// Similar to Phantom but using Solflare's API
document.getElementById('connectSolflare').onclick = async () => {
    try {
        const response = await window.solflare.connect();
        console.log('Connected with public key:', response.publicKey.toString());
        
        // Send to backend
        await fetch('/api/v1/wallets/connect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                wallet_type: 'solflare',
                public_key: response.publicKey.toString()
            })
        });
    } catch (error) {
        console.error('Failed to connect Solflare:', error);
    }
};
```

## Custom Wallet Providers

### Implementing Wallet Provider

Create a custom wallet provider by implementing the `WalletProvider` trait:

#### Rust Implementation
```rust
use async_trait::async_trait;
use solana_recover::wallet::{WalletProvider, WalletConnection, WalletCredentials};

pub struct CustomWalletProvider {
    client: reqwest::Client,
    api_url: String,
}

#[async_trait]
impl WalletProvider for CustomWalletProvider {
    async fn connect(&self, credentials: &WalletCredentials) -> Result<WalletConnection> {
        // Implement your wallet connection logic
        todo!()
    }

    async fn get_public_key(&self, connection: &WalletConnection) -> Result<String> {
        // Implement public key retrieval
        todo!()
    }

    async fn sign_transaction(&self, connection: &WalletConnection, transaction: &[u8]) -> Result<Vec<u8>> {
        // Implement transaction signing
        todo!()
    }

    async fn disconnect(&self, connection: &WalletConnection) -> Result<()> {
        // Implement wallet disconnection
        todo!()
    }
}
```

#### JavaScript Implementation
```javascript
class CustomWalletProvider {
    constructor(config) {
        this.config = config;
        this.client = new HttpClient();
    }

    async connect(credentials) {
        // Implement your wallet connection logic
        const response = await this.client.post('/auth', credentials);
        return {
            id: response.connectionId,
            walletType: 'custom',
            connectionData: response.sessionToken,
            createdAt: new Date().toISOString()
        };
    }

    async getPublicKey(connection) {
        // Implement public key retrieval
        const response = await this.client.get('/public-key', {
            headers: { 'Authorization': `Bearer ${connection.connectionData}` }
        });
        return response.publicKey;
    }

    async signTransaction(connection, transaction) {
        // Implement transaction signing
        const response = await this.client.post('/sign', {
            transaction: Array.from(transaction).toString(),
            sessionToken: connection.connectionData
        });
        return Buffer.from(response.signature, 'hex');
    }

    async disconnect(connection) {
        // Implement wallet disconnection
        await this.client.post('/logout', {
            sessionToken: connection.connectionData
        });
    }
}
```

### Register Custom Provider

```toml
[wallet.custom]
provider_class = "CustomWalletProvider"
enabled = true
config_file = "custom_wallet_config.toml"
```

## Security Best Practices

### Credential Management

1. **Environment Variables**
   - Never hardcode credentials in code
   - Use environment variables or secret management
   - Rotate credentials regularly

2. **Secret Storage**
   ```bash
   # Use a secret manager
   aws secretsmanager get-secret-value --secret-id solana-recover/turnkey
   
   # Or use Kubernetes secrets
   kubectl create secret generic turnkey-credentials \
     --from-literal=api-key=your-key \
     --from-literal=org-id=your-org
   ```

3. **Access Control**
   - Implement principle of least privilege
   - Use separate credentials for dev/staging/production
   - Monitor credential usage

### Transaction Security

1. **Validation**
   ```rust
   // Validate transaction before signing
   fn validate_transaction(tx: &Transaction) -> Result<()> {
       // Check destination address
       if !ALLOWED_DESTINATIONS.contains(&tx.destination) {
           return Err(SecurityError::InvalidDestination);
       }
       
       // Check amount limits
       if tx.amount > MAX_TRANSACTION_AMOUNT {
           return Err(SecurityError::AmountExceeded);
       }
       
       Ok(())
   }
   ```

2. **Audit Trail**
   ```rust
   // Log all signing operations
   async fn sign_transaction_with_audit(
       &self,
       connection: &WalletConnection,
       transaction: &[u8]
   ) -> Result<Vec<u8>> {
       let audit_log = AuditLog {
           timestamp: Utc::now(),
           wallet_connection_id: connection.id.clone(),
           transaction_hash: sha256(transaction),
           user_id: connection.user_id.clone(),
       };
       
       // Log before signing
       log::info!("Signing transaction: {:?}", audit_log);
       
       let signature = self.sign_transaction(connection, transaction).await?;
       
       // Log after signing
       log::info!("Transaction signed: {:?}", signature);
       
       Ok(signature)
   }
   ```

### Network Security

1. **HTTPS Only**
   - Always use HTTPS in production
   - Implement proper TLS configuration
   - Use certificate pinning for high-security applications

2. **Rate Limiting**
   ```toml
   [security.rate_limiting]
   enabled = true
   requests_per_minute = 60
   burst_size = 10
   wallet_specific_limits = true
   ```

## Troubleshooting

### Common Issues

#### Turnkey Connection Failed
```
Error: Turnkey auth request failed: Invalid credentials
```

**Solutions:**
1. Verify API key and organization ID
2. Check network connectivity to Turnkey API
3. Ensure credentials have proper permissions

#### Phantom Not Detected
```
Error: Phantom wallet not detected
```

**Solutions:**
1. Ensure Phantom extension is installed and enabled
2. Check if user has granted permissions
3. Verify your site is served over HTTPS

#### Transaction Signing Failed
```
Error: Transaction signing failed: User rejected
```

**Solutions:**
1. Check if user cancelled the transaction
2. Verify transaction format is correct
3. Ensure wallet has sufficient SOL for fees

### Debug Mode

Enable detailed logging for wallet operations:

```toml
[logging]
level = "debug"
modules = [
    "solana_recover::wallet=debug",
    "solana_recover::wallet::turnkey=trace",
    "solana_recover::wallet::phantom=debug"
]
```

### Testing

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_wallet_connection() {
        let provider = TurnkeyProvider::new();
        let credentials = create_test_credentials();
        
        let result = provider.connect(&credentials).await;
        assert!(result.is_ok());
    }
}
```

#### Integration Tests
```bash
# Test wallet integration
cargo test --test integration --features test-wallets

# Test with mock Turnkey server
TURNKEY_API_URL="http://localhost:3001" cargo test
```

---

This wallet integration guide provides comprehensive information for connecting various wallet providers to Solana Recover. For additional support, check the [API Documentation](api.md) or contact our support team.
