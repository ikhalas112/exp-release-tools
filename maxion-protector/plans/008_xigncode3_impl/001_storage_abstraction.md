# 001: Storage Abstraction Layer Specification

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Intermediate |
| Time to Read | 15 minutes |
| Audience | Developers, Architects, Database Engineers |

## Overview

This document defines the storage abstraction layer that decouples the anti-cheat detection system from specific storage implementations. This provides flexibility, testability, and the ability to switch between different backends without changing business logic.

## Architecture Goals

1. **Decoupling**: Business logic should not depend on concrete storage implementations
2. **Testability**: Easy to mock storage for unit tests
3. **Flexibility**: Support multiple storage backends (Cloudflare KV, Durable Objects, PostgreSQL, Redis)
4. **Performance**: Zero-abstraction overhead where possible
5. **SOLID Compliance**: Follow interface segregation and dependency inversion principles

## Crate Structure

```
maxion-detection-core/
├── Cargo.toml
└── src/
    ├── lib.rs                    # Main export module
    ├── mod.rs                    # Module index
    ├── storage/
    │   ├── mod.rs                # Storage module index
    │   ├── traits.rs             # Storage trait definitions
    │   ├── kv_backend.rs         # Cloudflare KV implementation
    │   ├── do_backend.rs         # Durable Objects implementation
    │   ├── pg_backend.rs         # PostgreSQL implementation (libsql)
    │   └── memory_backend.rs     # In-memory implementation (for testing)
    └── types.rs                  # Storage-specific types
```

## Cargo.toml

```toml
[package]
name = "maxion-detection-core"
version = "0.1.0"
edition = "2021"
authors = ["Maxion Team"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/maxion-game/maxion-protector"
description = "Core storage abstraction for Maxion anti-cheat detection"

[dependencies]
# Shared types
maxion-detection-types = { path = "../maxion-detection-types" }

# Workspace dependencies
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Database support
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "chrono", "uuid"] }

# Caching
moka = { version = "0.12", features = ["future"] }

# Cloudflare specific (optional, feature-gated)
worker = { version = "0.0.18", optional = true }

[features]
default = ["memory"]
cloudflare = ["worker"]
postgres = ["sqlx"]
memory = []
```

## Trait Definitions (src/storage/traits.rs)

```rust
//! Storage trait definitions for abstraction layer

use async_trait::async_trait;
use maxion_detection_types::{
    PlayerState, CheatEvent, BanRecord, SecurityPattern,
    ActionToken, DetectionResult, Result,
};
use std::collections::HashMap;

/// Main storage trait for anti-cheat operations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    // Player state operations
    async fn get_player_state(&self, player_id: &str) -> Result<Option<PlayerState>>;
    async fn update_player_state(&self, state: &PlayerState) -> Result<()>;
    async fn delete_player_state(&self, player_id: &str) -> Result<()>;
    
    // Cheat event operations
    async fn record_cheat_event(&self, event: &CheatEvent) -> Result<()>;
    async fn get_player_events(
        &self,
        player_id: &str,
        limit: usize,
    ) -> Result<Vec<CheatEvent>>;
    
    // Action token operations
    async fn create_action_token(&self, token: &ActionToken) -> Result<()>;
    async fn validate_action_token(&self, token_hash: &str) -> Result<bool>;
    async fn consume_action_token(&self, token_hash: &str) -> Result<()>;
    
    // Nonce operations (replay prevention)
    async fn check_nonce(&self, nonce: &str) -> Result<bool>;
    async fn mark_nonce_used(&self, nonce: &str, expiry_seconds: u64) -> Result<()>;
    
    // Ban operations
    async fn record_ban(&self, ban: &BanRecord) -> Result<()>;
    async fn get_active_ban(&self, player_id: &str) -> Result<Option<BanRecord>>;
    async fn update_ban_status(&self, ban_id: &str, status: &str) -> Result<()>;
    
    // Pattern operations
    async fn get_active_patterns(&self) -> Result<Vec<SecurityPattern>>;
    async fn add_pattern(&self, pattern: &SecurityPattern) -> Result<()>;
    async fn update_pattern(&self, pattern: &SecurityPattern) -> Result<()>;
    
    // Query operations
    async fn query_players_by_prefix(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<PlayerState>>;
    
    // Health check
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Storage health status
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Configuration for storage backends
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend_type: StorageType,
    pub connection_string: Option<String>,
    pub cache_ttl_seconds: Option<u64>,
    pub max_connections: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageType {
    Memory,
    CloudflareKV,
    DurableObject,
    PostgreSQL,
    Hybrid { primary: Box<StorageType>, fallback: Box<StorageType> },
}

impl StorageConfig {
    pub fn memory() -> Self {
        Self {
            backend_type: StorageType::Memory,
            connection_string: None,
            cache_ttl_seconds: None,
            max_connections: None,
        }
    }
    
    pub fn postgres(connection_string: String) -> Self {
        Self {
            backend_type: StorageType::PostgreSQL,
            connection_string: Some(connection_string),
            cache_ttl_seconds: Some(300),
            max_connections: Some(10),
        }
    }
}
```

## In-Memory Implementation (src/storage/memory_backend.rs)

```rust
//! In-memory storage backend for testing and development

use super::traits::{StorageBackend, HealthStatus, StorageConfig};
use maxion_detection_types::{
    PlayerState, CheatEvent, BanRecord, SecurityPattern,
    ActionToken, Result, DetectionError,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{Utc, Duration};
use std::time::Instant;

pub struct MemoryBackend {
    config: StorageConfig,
    player_states: Arc<RwLock<HashMap<String, PlayerState>>>,
    cheat_events: Arc<RwLock<HashMap<String, Vec<CheatEvent>>>>,
    action_tokens: Arc<RwLock<HashMap<String, (ActionToken, Instant)>>>,
    nonces: Arc<RwLock<HashMap<String, Instant>>>,
    bans: Arc<RwLock<HashMap<String, BanRecord>>>,
    patterns: Arc<RwLock<Vec<SecurityPattern>>>,
}

impl MemoryBackend {
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            player_states: Arc::new(RwLock::new(HashMap::new())),
            cheat_events: Arc::new(RwLock::new(HashMap::new())),
            action_tokens: Arc::new(RwLock::new(HashMap::new())),
            nonces: Arc::new(RwLock::new(HashMap::new())),
            bans: Arc::new(RwLock::new(HashMap::new())),
            patterns: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl StorageBackend for MemoryBackend {
    // Player state operations
    async fn get_player_state(&self, player_id: &str) -> Result<Option<PlayerState>> {
        let states = self.player_states.read().await;
        Ok(states.get(player_id).cloned())
    }
    
    async fn update_player_state(&self, state: &PlayerState) -> Result<()> {
        let mut states = self.player_states.write().await;
        states.insert(state.player_uuid.clone(), state.clone());
        Ok(())
    }
    
    async fn delete_player_state(&self, player_id: &str) -> Result<()> {
        let mut states = self.player_states.write().await;
        states.remove(player_id);
        Ok(())
    }
    
    // Cheat event operations
    async fn record_cheat_event(&self, event: &CheatEvent) -> Result<()> {
        let mut events = self.cheat_events.write().await;
        events
            .entry(event.player_id.clone())
            .or_insert_with(Vec::new)
            .push(event.clone());
        Ok(())
    }
    
    async fn get_player_events(
        &self,
        player_id: &str,
        limit: usize,
    ) -> Result<Vec<CheatEvent>> {
        let events = self.cheat_events.read().await;
        Ok(events
            .get(player_id)
            .map(|e| e.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default())
    }
    
    // Action token operations
    async fn create_action_token(&self, token: &ActionToken) -> Result<()> {
        let mut tokens = self.action_tokens.write().await;
        let token_hash = hex::encode(&token.token_hash);
        tokens.insert(token_hash, (token.clone(), Instant::now()));
        Ok(())
    }
    
    async fn validate_action_token(&self, token_hash: &str) -> Result<bool> {
        let tokens = self.action_tokens.read().await;
        match tokens.get(token_hash) {
            Some((token, _)) => Ok(token.is_valid()),
            None => Ok(false),
        }
    }
    
    async fn consume_action_token(&self, token_hash: &str) -> Result<()> {
        let mut tokens = self.action_tokens.write().await;
        tokens.remove(token_hash);
        Ok(())
    }
    
    // Nonce operations
    async fn check_nonce(&self, nonce: &str) -> Result<bool> {
        let nonces = self.nonces.read().await;
        let now = Instant::now();
        
        // Clean up expired nonces (older than 5 minutes)
        if let Some(expiry) = nonces.get(nonce) {
            let is_valid = now.duration_since(*expiry) < Duration::seconds(300).to_std().unwrap();
            Ok(!is_valid)
        } else {
            Ok(true)
        }
    }
    
    async fn mark_nonce_used(&self, nonce: &str, _expiry_seconds: u64) -> Result<()> {
        let mut nonces = self.nonces.write().await;
        nonces.insert(nonce.to_string(), Instant::now());
        Ok(())
    }
    
    // Ban operations
    async fn record_ban(&self, ban: &BanRecord) -> Result<()> {
        let mut bans = self.bans.write().await;
        bans.insert(ban.ban_id.clone(), ban.clone());
        Ok(())
    }
    
    async fn get_active_ban(&self, player_id: &str) -> Result<Option<BanRecord>> {
        let bans = self.bans.read().await;
        let now = Utc::now();
        
        for ban in bans.values() {
            if ban.player_id == player_id && ban.status == "Active" {
                match ban.expires_at {
                    Some(expiry) if expiry > now => return Ok(Some(ban.clone())),
                    None => return Ok(Some(ban.clone())),
                    _ => continue,
                }
            }
        }
        Ok(None)
    }
    
    async fn update_ban_status(&self, ban_id: &str, status: &str) -> Result<()> {
        let mut bans = self.bans.write().await;
        if let Some(ban) = bans.get_mut(ban_id) {
            ban.status = status.parse().map_err(|_| {
                DetectionError::InvalidRequest(format!("Invalid ban status: {}", status))
            })?;
        }
        Ok(())
    }
    
    // Pattern operations
    async fn get_active_patterns(&self) -> Result<Vec<SecurityPattern>> {
        let patterns = self.patterns.read().await;
        Ok(patterns
            .iter()
            .filter(|p| p.status == "Active")
            .cloned()
            .collect())
    }
    
    async fn add_pattern(&self, pattern: &SecurityPattern) -> Result<()> {
        let mut patterns = self.patterns.write().await;
        patterns.push(pattern.clone());
        Ok(())
    }
    
    async fn update_pattern(&self, pattern: &SecurityPattern) -> Result<()> {
        let mut patterns = self.patterns.write().await;
        if let Some(p) = patterns.iter_mut().find(|p| p.pattern_id == pattern.pattern_id) {
            *p = pattern.clone();
        }
        Ok(())
    }
    
    // Query operations
    async fn query_players_by_prefix(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<PlayerState>> {
        let states = self.player_states.read().await;
        Ok(states
            .values()
            .filter(|s| s.player_uuid.starts_with(prefix))
            .take(limit)
            .cloned()
            .collect())
    }
    
    // Health check
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_player_state_crud() {
        let backend = MemoryBackend::new(StorageConfig::memory());
        let player_id = "test-player-123";
        
        // Create
        let state = PlayerState {
            player_uuid: player_id.to_string(),
            violation_count: 1,
            last_violation: Utc::now(),
            status: maxion_detection_types::PlayerStatus::Flagged,
            recent_events: vec![],
            first_violation: Some(Utc::now()),
            updated_at: Utc::now(),
        };
        backend.update_player_state(&state).await.unwrap();
        
        // Read
        let retrieved = backend.get_player_state(player_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().player_uuid, player_id);
        
        // Delete
        backend.delete_player_state(player_id).await.unwrap();
        let retrieved = backend.get_player_state(player_id).await.unwrap();
        assert!(retrieved.is_none());
    }
    
    #[tokio::test]
    async fn test_nonce_replay_prevention() {
        let backend = MemoryBackend::new(StorageConfig::memory());
        let nonce = "test-nonce-123";
        
        // First check - nonce not used
        assert!(backend.check_nonce(nonce).await.unwrap());
        
        // Mark as used
        backend.mark_nonce_used(nonce, 300).await.unwrap();
        
        // Second check - nonce already used
        assert!(!backend.check_nonce(nonce).await.unwrap());
    }
}
```

## PostgreSQL Implementation (src/storage/pg_backend.rs)

```rust
//! PostgreSQL storage backend using libsql

use super::traits::{StorageBackend, HealthStatus, StorageConfig};
use maxion_detection_types::{
    PlayerState, CheatEvent, BanRecord, SecurityPattern,
    ActionToken, Result, DetectionError,
};
use sqlx::{PgPool, Row};
use chrono::Utc;

pub struct PostgresBackend {
    config: StorageConfig,
    pool: PgPool,
}

impl PostgresBackend {
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let connection_string = config.connection_string.as_ref().ok_or_else(|| {
            DetectionError::InvalidRequest("PostgreSQL connection string required".to_string())
        })?;
        
        let pool = PgPool::connect(connection_string).await.map_err(|e| {
            DetectionError::StorageError(format!("Failed to connect to PostgreSQL: {}", e))
        })?;
        
        Ok(Self { config, pool })
    }
    
    async fn init_schema(&self) -> Result<()> {
        sqlx::raw_sql(
            r#"
            CREATE TABLE IF NOT EXISTS player_states (
                player_uuid VARCHAR(255) PRIMARY KEY,
                violation_count INTEGER NOT NULL DEFAULT 0,
                last_violation TIMESTAMP NOT NULL,
                status VARCHAR(50) NOT NULL,
                recent_events JSONB NOT NULL DEFAULT '[]'::jsonb,
                first_violation TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_player_states_status ON player_states(status);
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(format!("Failed to init schema: {}", e)))?;
        
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    async fn get_player_state(&self, player_id: &str) -> Result<Option<PlayerState>> {
        let row = sqlx::raw_sql(&format!(
            "SELECT * FROM player_states WHERE player_uuid = '{}'",
            player_id
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        match row {
            Some(row) => {
                let state = PlayerState {
                    player_uuid: row.get("player_uuid"),
                    violation_count: row.get("violation_count"),
                    last_violation: row.get("last_violation"),
                    status: row.get("status"),
                    recent_events: serde_json::from_value(row.get("recent_events"))
                        .unwrap_or_default(),
                    first_violation: row.get("first_violation"),
                    updated_at: row.get("updated_at"),
                };
                Ok(Some(state))
            }
            None => Ok(None),
        }
    }
    
    async fn update_player_state(&self, state: &PlayerState) -> Result<()> {
        let recent_events_json = serde_json::to_value(&state.recent_events)
            .map_err(|e| DetectionError::SerializationError(e.to_string()))?;
        
        sqlx::raw_sql(&format!(
            r#"
            INSERT INTO player_states 
            (player_uuid, violation_count, last_violation, status, recent_events, 
             first_violation, updated_at)
            VALUES ('{}', {}, '{}', '{}', '{}', '{}', '{}')
            ON CONFLICT (player_uuid) DO UPDATE SET
                violation_count = EXCLUDED.violation_count,
                last_violation = EXCLUDED.last_violation,
                status = EXCLUDED.status,
                recent_events = EXCLUDED.recent_events,
                first_violation = COALESCE(EXCLUDED.first_violation, player_states.first_violation),
                updated_at = EXCLUDED.updated_at
            "#,
            state.player_uuid,
            state.violation_count,
            state.last_violation.format("%Y-%m-%d %H:%M:%S"),
            state.status,
            recent_events_json,
            state.first_violation.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or("NULL".to_string()),
            state.updated_at.format("%Y-%m-%d %H:%M:%S"),
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn delete_player_state(&self, player_id: &str) -> Result<()> {
        sqlx::raw_sql(&format!(
            "DELETE FROM player_states WHERE player_uuid = '{}'",
            player_id
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn record_cheat_event(&self, event: &CheatEvent) -> Result<()> {
        // Store cheat event in separate table
        sqlx::raw_sql(&format!(
            r#"
            INSERT INTO cheat_events 
            (game_id, version, player_id, cheat_type, hwid, timestamp, detection_count)
            VALUES ('{}', '{}', '{}', '{}', '{}', '{}', {})
            "#,
            event.game_id,
            event.version,
            event.player_id,
            event.cheat_type,
            event.hwid,
            event.timestamp.format("%Y-%m-%d %H:%M:%S"),
            event.detection_count,
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_player_events(
        &self,
        player_id: &str,
        limit: usize,
    ) -> Result<Vec<CheatEvent>> {
        let rows = sqlx::raw_sql(&format!(
            "SELECT * FROM cheat_events WHERE player_id = '{}' ORDER BY timestamp DESC LIMIT {}",
            player_id, limit
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        let events: Result<Vec<CheatEvent>> = rows
            .iter()
            .map(|row| {
                Ok(CheatEvent {
                    game_id: row.get("game_id"),
                    version: row.get("version"),
                    player_id: row.get("player_id"),
                    cheat_type: row.get("cheat_type"),
                    hwid: row.get("hwid"),
                    timestamp: row.get("timestamp"),
                    detection_count: row.get("detection_count"),
                })
            })
            .collect();
        
        events
    }
    
    async fn create_action_token(&self, token: &ActionToken) -> Result<()> {
        sqlx::raw_sql(&format!(
            r#"
            INSERT INTO action_tokens 
            (token_hash, player_id, timestamp, nonce, expires_at)
            VALUES ('{}', '{}', '{}', '{}', '{}')
            "#,
            hex::encode(&token.token_hash),
            token.player_id,
            token.timestamp,
            token.nonce,
            token.expires_at,
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn validate_action_token(&self, token_hash: &str) -> Result<bool> {
        let row = sqlx::raw_sql(&format!(
            "SELECT * FROM action_tokens WHERE token_hash = '{}' AND expires_at > NOW()",
            token_hash
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(row.is_some())
    }
    
    async fn consume_action_token(&self, token_hash: &str) -> Result<()> {
        sqlx::raw_sql(&format!(
            "DELETE FROM action_tokens WHERE token_hash = '{}'",
            token_hash
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn check_nonce(&self, nonce: &str) -> Result<bool> {
        let row = sqlx::raw_sql(&format!(
            "SELECT * FROM used_nonces WHERE nonce = '{}' AND expiry > NOW()",
            nonce
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(row.is_none())
    }
    
    async fn mark_nonce_used(&self, nonce: &str, expiry_seconds: u64) -> Result<()> {
        sqlx::raw_sql(&format!(
            "INSERT INTO used_nonces (nonce, expiry) VALUES ('{}', NOW() + INTERVAL '{} seconds')",
            nonce, expiry_seconds
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn record_ban(&self, ban: &BanRecord) -> Result<()> {
        sqlx::raw_sql(&format!(
            r#"
            INSERT INTO bans 
            (ban_id, player_id, ban_type, reason, status, created_at, expires_at, created_by)
            VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}')
            "#,
            ban.ban_id,
            ban.player_id,
            serde_json::to_string(&ban.ban_type).unwrap(),
            serde_json::to_string(&ban.reason).unwrap(),
            ban.status,
            ban.created_at.format("%Y-%m-%d %H:%M:%S"),
            ban.expires_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or("NULL".to_string()),
            ban.created_by,
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_active_ban(&self, player_id: &str) -> Result<Option<BanRecord>> {
        let row = sqlx::raw_sql(&format!(
            r#"
            SELECT * FROM bans 
            WHERE player_id = '{}' AND status = 'Active' 
            AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            player_id
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        match row {
            Some(row) => {
                let ban = BanRecord {
                    ban_id: row.get("ban_id"),
                    player_id: row.get("player_id"),
                    ban_type: serde_json::from_str(row.get("ban_type")).unwrap(),
                    reason: serde_json::from_str(row.get("reason")).unwrap(),
                    status: row.get("status"),
                    created_at: row.get("created_at"),
                    expires_at: row.get("expires_at"),
                    created_by: row.get("created_by"),
                    metadata: maxion_detection_types::BanMetadata {
                        hardware_id: None,
                        ip_address: None,
                        game_sessions_affected: 0,
                        evidence_urls: vec![],
                        notes: String::new(),
                    },
                };
                Ok(Some(ban))
            }
            None => Ok(None),
        }
    }
    
    async fn update_ban_status(&self, ban_id: &str, status: &str) -> Result<()> {
        sqlx::raw_sql(&format!(
            "UPDATE bans SET status = '{}' WHERE ban_id = '{}'",
            status, ban_id
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn get_active_patterns(&self) -> Result<Vec<SecurityPattern>> {
        let rows = sqlx::raw_sql(
            "SELECT * FROM security_patterns WHERE status = 'Active' ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        let patterns: Result<Vec<SecurityPattern>> = rows
            .iter()
            .map(|row| {
                Ok(SecurityPattern {
                    pattern_id: row.get("pattern_id"),
                    pattern_type: row.get("pattern_type"),
                    version: row.get("version"),
                    signature: row.get("signature"),
                    metadata: serde_json::from_str(row.get("metadata")).unwrap(),
                    effectiveness: serde_json::from_str(row.get("effectiveness")).unwrap(),
                    status: row.get("status"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    expires_at: row.get("expires_at"),
                })
            })
            .collect();
        
        patterns
    }
    
    async fn add_pattern(&self, pattern: &SecurityPattern) -> Result<()> {
        sqlx::raw_sql(&format!(
            r#"
            INSERT INTO security_patterns 
            (pattern_id, pattern_type, version, signature, metadata, effectiveness, status, 
             created_at, updated_at, expires_at)
            VALUES ('{}', '{}', {}, '{}', '{}', '{}', '{}', '{}', '{}', '{}')
            "#,
            pattern.pattern_id,
            pattern.pattern_type,
            pattern.version,
            hex::encode(&pattern.signature),
            serde_json::to_string(&pattern.metadata).unwrap(),
            serde_json::to_string(&pattern.effectiveness).unwrap(),
            pattern.status,
            pattern.created_at.format("%Y-%m-%d %H:%M:%S"),
            pattern.updated_at.format("%Y-%m-%d %H:%M:%S"),
            pattern.expires_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or("NULL".to_string()),
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn update_pattern(&self, pattern: &SecurityPattern) -> Result<()> {
        sqlx::raw_sql(&format!(
            r#"
            UPDATE security_patterns SET
                pattern_type = '{}',
                version = {},
                signature = '{}',
                metadata = '{}',
                effectiveness = '{}',
                status = '{}',
                updated_at = '{}',
                expires_at = '{}'
            WHERE pattern_id = '{}'
            "#,
            pattern.pattern_type,
            pattern.version,
            hex::encode(&pattern.signature),
            serde_json::to_string(&pattern.metadata).unwrap(),
            serde_json::to_string(&pattern.effectiveness).unwrap(),
            pattern.status,
            pattern.updated_at.format("%Y-%m-%d %H:%M:%S"),
            pattern.expires_at.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or("NULL".to_string()),
            pattern.pattern_id,
        ))
        .execute(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn query_players_by_prefix(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<PlayerState>> {
        let rows = sqlx::raw_sql(&format!(
            "SELECT * FROM player_states WHERE player_uuid LIKE '{}%' LIMIT {}",
            prefix, limit
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DetectionError::StorageError(e.to_string()))?;
        
        let states: Result<Vec<PlayerState>> = rows
            .iter()
            .map(|row| {
                Ok(PlayerState {
                    player_uuid: row.get("player_uuid"),
                    violation_count: row.get("violation_count"),
                    last_violation: row.get("last_violation"),
                    status: row.get("status"),
                    recent_events: serde_json::from_value(row.get("recent_events"))
                        .unwrap_or_default(),
                    first_violation: row.get("first_violation"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect();
        
        states
    }
    
    async fn health_check(&self) -> Result<HealthStatus> {
        sqlx::raw_sql("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| HealthStatus::Healthy)
            .map_err(|e| DetectionError::StorageError(format!("Health check failed: {}", e)))
    }
}
```

## Factory Pattern (src/storage/mod.rs)

```rust
//! Storage module with factory for creating backends

pub mod traits;
pub mod memory_backend;
pub mod pg_backend;

use traits::{StorageBackend, StorageConfig, StorageType};

pub use traits::{HealthStatus, StorageBackend as Storage};

/// Factory for creating storage backends
pub struct StorageFactory;

impl StorageFactory {
    pub async fn create(config: StorageConfig) -> Result<Box<dyn StorageBackend>> {
        match config.backend_type {
            StorageType::Memory => {
                let backend = memory_backend::MemoryBackend::new(config);
                Ok(Box::new(backend))
            }
            StorageType::PostgreSQL => {
                let backend = pg_backend::PostgresBackend::new(config).await?;
                Ok(Box::new(backend))
            }
            _ => Err(maxion_detection_types::DetectionError::InvalidRequest(
                format!("Unsupported storage type: {:?}", config.backend_type),
            )),
        }
    }
}

/// Get default storage backend for the current environment
pub async fn default_storage() -> Result<Box<dyn StorageBackend>> {
    #[cfg(feature = "memory")]
    {
        StorageFactory::create(StorageConfig::memory()).await
    }
    
    #[cfg(feature = "postgres")]
    {
        let conn_str = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/maxion".to_string());
        StorageFactory::create(StorageConfig::postgres(conn_str)).await
    }
    
    #[cfg(all(not(feature = "memory"), not(feature = "postgres")))]
    {
        Err(maxion_detection_types::DetectionError::InvalidRequest(
            "No storage feature enabled".to_string(),
        ))
    }
}
```

## Usage Examples

### Example 1: Using In-Memory Storage (Testing)

```rust
use maxion_detection_core::storage::{StorageFactory, StorageConfig};

#[tokio::test]
async fn test_cheat_detection() {
    let config = StorageConfig::memory();
    let storage = StorageFactory::create(config).await.unwrap();
    
    // Use storage through trait
    let player_id = "test-player-123";
    let state = PlayerState {
        player_uuid: player_id.to_string(),
        violation_count: 1,
        last_violation: Utc::now(),
        status: PlayerStatus::Flagged,
        recent_events: vec![],
        first_violation: Some(Utc::now()),
        updated_at: Utc::now(),
    };
    
    storage.update_player_state(&state).await.unwrap();
    let retrieved = storage.get_player_state(player_id).await.unwrap();
    assert!(retrieved.is_some());
}
```

### Example 2: Using PostgreSQL Storage (Production)

```rust
use maxion_detection_core::storage::{StorageFactory, StorageConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let config = StorageConfig::postgres("postgresql://localhost/maxion".to_string());
    let storage = StorageFactory::create(config).await?;
    
    // Record cheat event
    let event = CheatEvent {
        game_id: "my-game".to_string(),
        version: "1.0.0".to_string(),
        player_id: "player-123".to_string(),
        cheat_type: CheatType::ProcessInjection,
        hwid: "hwid-456".to_string(),
        timestamp: Utc::now(),
        detection_count: 1,
    };
    
    storage.record_cheat_event(&event).await?;
    
    // Check for active ban
    if let Some(ban) = storage.get_active_ban("player-123").await? {
        println!("Player is banned: {:?}", ban);
    }
    
    Ok(())
}
```

### Example 3: Dependency Injection in Services

```rust
use maxion_detection_core::storage::Storage;

pub struct DetectionService {
    storage: Box<dyn Storage>,
}

impl DetectionService {
    pub fn new(storage: Box<dyn Storage>) -> Self {
        Self { storage }
    }
    
    pub async fn process_cheat_event(&self, event: &CheatEvent) -> Result<()> {
        // Record event through storage abstraction
        self.storage.record_cheat_event(event).await?;
        
        // Update player state
        let mut state = self.storage.get_player_state(&event.player_id).await?
            .unwrap_or_else(|| PlayerState {
                player_uuid: event.player_id.clone(),
                violation_count: 0,
                last_violation: Utc::now(),
                status: PlayerStatus::Clean,
                recent_events: vec![],
                first_violation: None,
                updated_at: Utc::now(),
            });
        
        state.violation_count += 1;
        state.last_violation = event.timestamp;
        state.status = PlayerStatus::Flagged;
        state.recent_events.push(event.clone());
        
        self.storage.update_player_state(&state).await?;
        
        Ok(())
    }
}
```

## Benefits of Storage Abstraction

1. **Flexibility**: Switch between storage backends without changing business logic
2. **Testability**: Use in-memory backend for fast, isolated tests
3. **Decoupling**: Business logic depends on traits, not concrete implementations
4. **Scalability**: Easy to add new storage backends (Redis, Cassandra, etc.)
5. **Performance**: Optimize per-backend without affecting other components
6. **Migration**: Gradual migration from one backend to another
7. **Development**: Can develop with in-memory storage, deploy with PostgreSQL

## Migration Guide

### Phase 1: Define Abstraction
1. Create `maxion-detection-core` crate
2. Define `StorageBackend` trait
3. Implement in-memory backend for testing

### Phase 2: Migrate Services
1. Update 008b, 008c to use `StorageBackend` trait
2. Replace direct KV/DO calls with trait methods
3. Add dependency injection to services

### Phase 3: Add Implementations
1. Implement PostgreSQL backend (for production)
2. Implement Cloudflare KV backend (for Workers)
3. Implement Durable Objects backend (for state management)

### Phase 4: Optimize
1. Add caching layer (Moka) for frequently accessed data
2. Optimize queries per backend
3. Add connection pooling
4. Implement hybrid storage (primary + fallback)

## Next Steps

1. [ ] Review and approve storage trait definitions
2. [ ] Implement all storage backends
3. [ ] Add comprehensive unit tests for each backend
4. [ ] Add integration tests with PostgreSQL
5. [ ] Update phase documents (008b, 008c, 008d, 008e, 008f) to use storage abstraction
6. [ ] Remove direct storage access from phase documents
7. [ ] Create migration guide for existing code
8. [ ] Add performance benchmarks for each backend

## Notes

- All storage operations use `sqlx::raw_sql` for compatibility with PgCat connection pooler
- Timestamps use `chrono::DateTime<Utc>` for consistency
- Errors use `maxion_detection_types::DetectionError` for unified error handling
- In-memory backend is primarily for testing and development
- PostgreSQL backend uses libsql for encryption support
- Cloudflare-specific backends are feature-gated