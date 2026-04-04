use crate::core::{Result, SolanaRecoverError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use rusqlite::{Connection, params, Row};
use std::sync::{Arc, Mutex};
use tokio::task;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite:./solana_recover.db".to_string(),
            max_connections: 10,
        }
    }
}

#[async_trait]
pub trait PersistenceManager: Send + Sync {
    async fn save_scan_result(&self, result: &crate::core::ScanResult) -> Result<()>;
    async fn get_scan_result(&self, scan_id: &str) -> Result<Option<crate::core::ScanResult>>;
    async fn save_wallet_info(&self, info: &crate::core::WalletInfo) -> Result<()>;
    async fn get_wallet_info(&self, address: &str) -> Result<Option<crate::core::WalletInfo>>;
    async fn save_user(&self, user: &crate::core::User) -> Result<()>;
    async fn get_user(&self, user_id: &str) -> Result<Option<crate::core::User>>;
    async fn get_user_wallets(&self, user_id: &str) -> Result<Vec<String>>;
    async fn cleanup_old_records(&self, days_old: u32) -> Result<u64>;
}

pub struct SqlitePersistenceManager {
    pool: SqlitePool,
}

impl SqlitePersistenceManager {
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(&config.database_url)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Invalid database URL: {}", e)
            ))?
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to connect to database: {}", e)
            ))?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        // Create tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                created_at DATETIME NOT NULL,
                last_active DATETIME,
                metadata TEXT
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create users table: {}", e)
        ))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scan_results (
                id TEXT PRIMARY KEY,
                wallet_address TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                error TEXT,
                created_at DATETIME NOT NULL,
                user_id TEXT,
                FOREIGN KEY (user_id) REFERENCES users (id)
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create scan_results table: {}", e)
        ))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wallet_info (
                address TEXT PRIMARY KEY,
                pubkey TEXT NOT NULL,
                total_accounts INTEGER NOT NULL,
                empty_accounts INTEGER NOT NULL,
                recoverable_lamports INTEGER NOT NULL,
                recoverable_sol REAL NOT NULL,
                empty_account_addresses TEXT,
                scan_time_ms INTEGER NOT NULL,
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create wallet_info table: {}", e)
        ))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_wallets (
                user_id TEXT NOT NULL,
                wallet_address TEXT NOT NULL,
                added_at DATETIME NOT NULL,
                PRIMARY KEY (user_id, wallet_address),
                FOREIGN KEY (user_id) REFERENCES users (id),
                FOREIGN KEY (wallet_address) REFERENCES wallet_info (address)
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create user_wallets table: {}", e)
        ))?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_scan_results_wallet ON scan_results(wallet_address)")
            .execute(pool)
            .await
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to create scan_results index: {}", e)
            ))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_scan_results_user ON scan_results(user_id)")
            .execute(pool)
            .await
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to create scan_results user index: {}", e)
            ))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_info_created ON wallet_info(created_at)")
            .execute(pool)
            .await
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to create wallet_info index: {}", e)
            ))?;

        Ok(())
    }
}

#[async_trait]
impl PersistenceManager for SqlitePersistenceManager {
    async fn save_scan_result(&self, result: &crate::core::ScanResult) -> Result<()> {
        let result_json = serde_json::to_string(result)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize scan result: {}", e)
            ))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO scan_results 
            (id, wallet_address, status, result, error, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(result.id.to_string())
        .bind(&result.wallet_address)
        .bind(serde_json::to_string(&result.status).unwrap())
        .bind(result.result.as_ref().map(|r| serde_json::to_string(r).unwrap()))
        .bind(&result.error)
        .bind(result.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to save scan result: {}", e)
        ))?;

        Ok(())
    }

    async fn get_scan_result(&self, scan_id: &str) -> Result<Option<crate::core::ScanResult>> {
        let row = sqlx::query(
            "SELECT result FROM scan_results WHERE id = ?"
        )
        .bind(scan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to get scan result: {}", e)
        ))?;

        if let Some(row) = row {
            let result_json: String = row.get("result");
            let result: crate::core::ScanResult = serde_json::from_str(&result_json)
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to deserialize scan result: {}", e)
                ))?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    async fn save_wallet_info(&self, info: &crate::core::WalletInfo) -> Result<()> {
        let addresses_json = serde_json::to_string(&info.empty_account_addresses)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize wallet addresses: {}", e)
            ))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO wallet_info 
            (address, pubkey, total_accounts, empty_accounts, recoverable_lamports, 
             recoverable_sol, empty_account_addresses, scan_time_ms, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(&info.address)
        .bind(info.pubkey.to_string())
        .bind(info.total_accounts as i64)
        .bind(info.empty_accounts as i64)
        .bind(info.recoverable_lamports as i64)
        .bind(info.recoverable_sol)
        .bind(addresses_json)
        .bind(info.scan_time_ms as i64)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to save wallet info: {}", e)
        ))?;

        Ok(())
    }

    async fn get_wallet_info(&self, address: &str) -> Result<Option<crate::core::WalletInfo>> {
        let row = sqlx::query(
            "SELECT * FROM wallet_info WHERE address = ?"
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to get wallet info: {}", e)
        ))?;

        if let Some(row) = row {
            let addresses_json: String = row.get("empty_account_addresses");
            let empty_account_addresses: Vec<String> = serde_json::from_str(&addresses_json)
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to deserialize wallet addresses: {}", e)
                ))?;

            let pubkey_str: String = row.get("pubkey");
            let pubkey = solana_sdk::pubkey::Pubkey::from_str(&pubkey_str)
                .map_err(|_| SolanaRecoverError::InvalidWalletAddress(pubkey_str))?;

            let info = crate::core::WalletInfo {
                address: row.get("address"),
                pubkey,
                total_accounts: row.get("total_accounts"),
                empty_accounts: row.get("empty_accounts"),
                recoverable_lamports: row.get("recoverable_lamports"),
                recoverable_sol: row.get("recoverable_sol"),
                empty_account_addresses,
                scan_time_ms: row.get("scan_time_ms"),
            };

            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    async fn save_user(&self, user: &crate::core::User) -> Result<()> {
        let metadata_json = serde_json::to_string(&user.metadata)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize user metadata: {}", e)
            ))?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO users 
            (id, email, created_at, last_active, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&user.id)
        .bind(&user.email)
        .bind(user.created_at)
        .bind(user.last_active)
        .bind(metadata_json)
        .execute(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to save user: {}", e)
        ))?;

        Ok(())
    }

    async fn get_user(&self, user_id: &str) -> Result<Option<crate::core::User>> {
        let row = sqlx::query(
            "SELECT * FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to get user: {}", e)
        ))?;

        if let Some(row) = row {
            let metadata_json: String = row.get("metadata");
            let metadata: serde_json::Value = serde_json::from_str(&metadata_json)
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to deserialize user metadata: {}", e)
                ))?;

            let user = crate::core::User {
                id: row.get("id"),
                email: row.get("email"),
                created_at: row.get("created_at"),
                last_active: row.get("last_active"),
                metadata,
            };

            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    async fn get_user_wallets(&self, user_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT wallet_address FROM user_wallets WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to get user wallets: {}", e)
        ))?;

        let wallets: Vec<String> = rows
            .iter()
            .map(|row| row.get("wallet_address"))
            .collect();

        Ok(wallets)
    }

    async fn cleanup_old_records(&self, days_old: u32) -> Result<u64> {
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days_old as i64);

        let result = sqlx::query(
            "DELETE FROM scan_results WHERE created_at < ?"
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to cleanup old records: {}", e)
        ))?;

        Ok(result.rows_affected())
    }
}
