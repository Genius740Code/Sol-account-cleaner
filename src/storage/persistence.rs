use crate::core::{Result, SolanaRecoverError};
use async_trait::async_trait;
use rusqlite::{Connection, params, OptionalExtension};
use std::sync::{Arc, Mutex};
use tokio::task;
use std::str::FromStr;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    conn: Arc<Mutex<Connection>>,
}

impl SqlitePersistenceManager {
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let conn = task::spawn_blocking(move || -> Result<Connection> {
            let conn = Connection::open(&config.database_url)
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to connect to database: {}", e)
                ))?;

            // Configure SQLite for better performance
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to set journal mode: {}", e)
                ))?;
            
            conn.pragma_update(None, "synchronous", "NORMAL")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to set synchronous mode: {}", e)
                ))?;

            // Run migrations
            Self::run_migrations(&conn)?;
            
            Ok(conn)
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Database initialization failed: {}", e)
        ))??;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn run_migrations(conn: &Connection) -> Result<()> {
        // Create tables
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                created_at TEXT NOT NULL,
                last_active TEXT,
                metadata TEXT
            )
            "#,
            [],
        )
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create users table: {}", e)
        ))?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS scan_results (
                id TEXT PRIMARY KEY,
                wallet_address TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                user_id TEXT,
                FOREIGN KEY (user_id) REFERENCES users (id)
            )
            "#,
            [],
        )
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create scan_results table: {}", e)
        ))?;

        conn.execute(
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
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            [],
        )
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create wallet_info table: {}", e)
        ))?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS user_wallets (
                user_id TEXT NOT NULL,
                wallet_address TEXT NOT NULL,
                added_at TEXT NOT NULL,
                PRIMARY KEY (user_id, wallet_address),
                FOREIGN KEY (user_id) REFERENCES users (id),
                FOREIGN KEY (wallet_address) REFERENCES wallet_info (address)
            )
            "#,
            [],
        )
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Failed to create user_wallets table: {}", e)
        ))?;

        // Create indexes
        conn.execute("CREATE INDEX IF NOT EXISTS idx_scan_results_wallet ON scan_results(wallet_address)", [])
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to create scan_results index: {}", e)
            ))?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_scan_results_user ON scan_results(user_id)", [])
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to create scan_results user index: {}", e)
            ))?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_wallet_info_created ON wallet_info(created_at)", [])
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

        let conn = self.conn.clone();
        let scan_id = result.id.to_string();
        let wallet_address = result.wallet_address.clone();
        let status = serde_json::to_string(&result.status).unwrap();
        let result_data = Some(result_json);
        let error = result.error.clone();
        let created_at = result.created_at.to_rfc3339();

        task::spawn_blocking(move || -> Result<()> {
            let conn = conn.lock().unwrap();
            
            conn.execute(
                r#"
                INSERT OR REPLACE INTO scan_results 
                (id, wallet_address, status, result, error, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    scan_id,
                    wallet_address,
                    status,
                    result_data,
                    error,
                    created_at
                ],
            )
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to save scan result: {}", e)
            ))?;

            Ok(())
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))?
    }

    async fn get_scan_result(&self, scan_id: &str) -> Result<Option<crate::core::ScanResult>> {
        let conn = self.conn.clone();
        let scan_id = scan_id.to_string();

        let result_json = task::spawn_blocking(move || -> Result<Option<String>> {
            let conn = conn.lock().unwrap();
            
            let mut stmt = conn
                .prepare("SELECT result FROM scan_results WHERE id = ?1")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to prepare statement: {}", e)
                ))?;

            let result: Option<String> = stmt
                .query_row(params![scan_id], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to query scan result: {}", e)
                ))?;

            Ok(result)
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))??;

        if let Some(result_json) = result_json {
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

        let conn = self.conn.clone();
        let address = info.address.clone();
        let pubkey = info.pubkey.to_string();
        let total_accounts = info.total_accounts as i64;
        let empty_accounts = info.empty_accounts as i64;
        let recoverable_lamports = info.recoverable_lamports as i64;
        let recoverable_sol = info.recoverable_sol;
        let scan_time_ms = info.scan_time_ms as i64;
        let now = chrono::Utc::now().to_rfc3339();

        task::spawn_blocking(move || -> Result<()> {
            let conn = conn.lock().unwrap();
            
            conn.execute(
                r#"
                INSERT OR REPLACE INTO wallet_info 
                (address, pubkey, total_accounts, empty_accounts, recoverable_lamports, 
                 recoverable_sol, empty_account_addresses, scan_time_ms, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    address,
                    pubkey,
                    total_accounts,
                    empty_accounts,
                    recoverable_lamports,
                    recoverable_sol,
                    addresses_json,
                    scan_time_ms,
                    now,
                    now
                ],
            )
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to save wallet info: {}", e)
            ))?;

            Ok(())
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))?
    }

    async fn get_wallet_info(&self, address: &str) -> Result<Option<crate::core::WalletInfo>> {
        let conn = self.conn.clone();
        let address = address.to_string();

        let wallet_info = task::spawn_blocking(move || -> Result<Option<crate::core::WalletInfo>> {
            let conn = conn.lock().unwrap();
            
            let mut stmt = conn
                .prepare("SELECT * FROM wallet_info WHERE address = ?1")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to prepare statement: {}", e)
                ))?;

            let result = stmt
                .query_row(params![address], |row| {
                    let addresses_json: String = row.get(8)?;
                    let empty_account_addresses: Vec<String> = serde_json::from_str(&addresses_json)
                        .map_err(|_e| rusqlite::Error::InvalidColumnType(
                            8, "empty_account_addresses".to_string(), rusqlite::types::Type::Text
                        ))?;

                    let pubkey_str: String = row.get(1)?;
                    let pubkey = solana_sdk::pubkey::Pubkey::from_str(&pubkey_str)
                        .map_err(|_| rusqlite::Error::InvalidColumnType(
                            1, "pubkey".to_string(), rusqlite::types::Type::Text
                        ))?;

                    Ok(crate::core::WalletInfo {
                        address: row.get(0)?,
                        pubkey,
                        total_accounts: row.get(2)?,
                        empty_accounts: row.get(3)?,
                        recoverable_lamports: row.get(4)?,
                        recoverable_sol: row.get(5)?,
                        empty_account_addresses,
                        scan_time_ms: row.get(7)?,
                    })
                })
                .optional()
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to query wallet info: {}", e)
                ))?;

            Ok(result)
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))??;

        Ok(wallet_info)
    }

    async fn save_user(&self, user: &crate::core::User) -> Result<()> {
        let metadata_json = serde_json::to_string(&user.metadata)
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to serialize user metadata: {}", e)
            ))?;

        let conn = self.conn.clone();
        let id = user.id.clone();
        let email = user.email.clone();
        let created_at = user.created_at.to_rfc3339();
        let last_active = user.last_active.map(|dt| dt.to_rfc3339());

        task::spawn_blocking(move || -> Result<()> {
            let conn = conn.lock().unwrap();
            
            conn.execute(
                r#"
                INSERT OR REPLACE INTO users 
                (id, email, created_at, last_active, metadata)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    id,
                    email,
                    created_at,
                    last_active,
                    metadata_json
                ],
            )
            .map_err(|e| SolanaRecoverError::StorageError(
                format!("Failed to save user: {}", e)
            ))?;

            Ok(())
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))?
    }

    async fn get_user(&self, user_id: &str) -> Result<Option<crate::core::User>> {
        let conn = self.conn.clone();
        let user_id = user_id.to_string();

        let user = task::spawn_blocking(move || -> Result<Option<crate::core::User>> {
            let conn = conn.lock().unwrap();
            
            let mut stmt = conn
                .prepare("SELECT * FROM users WHERE id = ?1")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to prepare statement: {}", e)
                ))?;

            let result = stmt
                .query_row(params![user_id], |row| {
                    let metadata_json: String = row.get(4)?;
                    let metadata: serde_json::Value = serde_json::from_str(&metadata_json)
                        .map_err(|_e| rusqlite::Error::InvalidColumnType(
                            4, "metadata".to_string(), rusqlite::types::Type::Text
                        ))?;

                    let last_active_str: Option<String> = row.get(3)?;
                    let last_active = last_active_str
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc));

                    Ok(crate::core::User {
                        id: row.get(0)?,
                        email: row.get(1)?,
                        api_key: None, // Not stored in this simple implementation
                        fee_structure: None, // Not stored in this simple implementation
                        rate_limit_rps: None, // Not stored in this simple implementation
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                            .map_err(|_e| rusqlite::Error::InvalidColumnType(
                                2, "created_at".to_string(), rusqlite::types::Type::Text
                            ))?
                            .with_timezone(&chrono::Utc),
                        last_active,
                        metadata,
                    })
                })
                .optional()
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to query user: {}", e)
                ))?;

            Ok(result)
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))??;

        Ok(user)
    }

    async fn get_user_wallets(&self, user_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.clone();
        let user_id = user_id.to_string();

        let wallets = task::spawn_blocking(move || -> Result<Vec<String>> {
            let conn = conn.lock().unwrap();
            
            let mut stmt = conn
                .prepare("SELECT wallet_address FROM user_wallets WHERE user_id = ?1")
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to prepare statement: {}", e)
                ))?;

            let wallet_iter = stmt
                .query_map(params![user_id], |row| {
                    row.get(0)
                })
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to query user wallets: {}", e)
                ))?;

            let wallets: Result<Vec<String>> = wallet_iter
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to collect wallets: {}", e)
                ));

            wallets
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))??;

        Ok(wallets)
    }

    async fn cleanup_old_records(&self, days_old: u32) -> Result<u64> {
        let conn = self.conn.clone();
        let cutoff_date = (chrono::Utc::now() - chrono::Duration::days(days_old as i64)).to_rfc3339();

        let rows_affected = task::spawn_blocking(move || -> Result<u64> {
            let conn = conn.lock().unwrap();
            
            let rows_affected = conn
                .execute("DELETE FROM scan_results WHERE created_at < ?1", params![cutoff_date])
                .map_err(|e| SolanaRecoverError::StorageError(
                    format!("Failed to cleanup old records: {}", e)
                ))?;

            Ok(rows_affected as u64)
        })
        .await
        .map_err(|e| SolanaRecoverError::StorageError(
            format!("Task execution failed: {}", e)
        ))??;

        Ok(rows_affected)
    }
}
