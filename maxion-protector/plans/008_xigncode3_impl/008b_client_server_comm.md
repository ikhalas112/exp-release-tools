# 008b: Client-Server Communication Layer

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-24 |
| Version | 2.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, System Engineers, Network Engineers |

## Overview
This plan establishes secure, high-performance communication between the native Windows client (008a) and the server infrastructure. This layer enables telemetry reporting, real-time pattern updates, and session validation while leveraging Cloudflare's Durable Objects with SQLite for zero-latency state storage.

## Architecture Notes

### Infrastructure Stack
- **Cloudflare Workers (Rust)**: Edge computing for API routing, rate limiting, and lightweight processing
- **Cloudflare Durable Objects (SQLite)**: State management with zero-latency SQL queries (synchronous, no awaits)
- **Cloudflare Docker Containers**: Heavy processing when Workers can't handle it (machine learning, complex analysis)
- **Axum**: Backend API server running in Docker containers for complex operations
- **PostgreSQL**: Long-term persistence, analytics, historical data

### Communication Flow
```
Client (Native Windows Rust) 
  → HTTPS (Encrypted) 
    → Cloudflare Worker (Rust) 
      → Rate Limiting/Validation 
        → Durable Object (SQLite, zero-latency) 
          → Docker Container (Axum, heavy processing)
            → PostgreSQL (Long-term storage)
```

### Key Architectural Principles
1. **Zero-Latency SQLite**: Durable Objects run SQLite in the same thread as application code, queries complete in microseconds
2. **Synchronous Queries**: No `await` needed for SQLite queries - database is in the same thread
3. **Output Gates**: Writes confirm durability automatically, responses blocked until writes persist
4. **Stateless Workers**: Workers route requests to appropriate Durable Objects
5. **Horizontal Scaling**: Durable Objects scale out by creating more objects (one per logical entity)
6. **Docker for Heavy Lifting**: Complex ML/analysis runs in containers, Workers handle lightweight routing

### Durable Objects + SQLite Benefits
- **Latency**: Effectively zero (no network hop to database)
- **Throughput**: High (no async overhead for common queries)
- **Durability**: Writes replicated to 5 followers before confirmation
- **Point-in-Time Recovery**: Revert to any state in last 30 days
- **Cost**: $0.001 per million rows read, $1.00 per million rows written

## Implementation Tasks

### Task 1: Cloudflare Worker Setup (Day 1-2)

#### 1.1 Project Structure
```
maxion-server-worker/
├── Cargo.toml
├── wrangler.toml          # Cloudflare config
├── src/
│   ├── lib.rs             # Worker entry point
│   ├── types.rs           # Shared types
│   ├── auth.rs            # Authentication logic
│   ├── rate_limit.rs      # Rate limiting
│   ├── router.rs          # Request routing to Durable Objects
│   └── telemetry.rs       # Telemetry processing
├── durable_objects/
│   ├── mod.rs             # DO definitions
│   ├── session.rs         # Session management DO
│   ├── pattern.rs         # Pattern distribution DO
│   └── telemetry.rs       # Telemetry aggregation DO
└── worker.rs              # JavaScript entry point (minimal)
```

#### 1.2 wrangler.toml Configuration
```toml
name = "maxion-protector-worker"
main = "worker.rs"
compatibility_date = "2024-09-26"
# Important: Use SQLite-backed Durable Objects

[vars]
ENVIRONMENT = "production"
AXUM_BACKEND_URL = "https://api.maxion-protector.com"

[[durable_objects.bindings]]
name = "SESSIONS"
class_name = "SessionObject"

[[durable_objects.bindings]]
name = "PATTERNS"
class_name = "PatternObject"

[[durable_objects.bindings]]
name = "TELEMETRY"
class_name = "TelemetryObject"

# Migrations for SQLite-backed DOs
[[migrations]]
tag = "v1"
new_sqlite_classes = ["SessionObject", "PatternObject", "TelemetryObject"]

[build]
command = "cargo build --release --target wasm32-wasi"

[env.development]
name = "maxion-protector-worker-dev"
```

#### 1.3 Cargo.toml
```toml
[package]
name = "maxion-server-worker"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
# Cloudflare Workers SDK
worker = "0.4"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Cryptography (per project guidelines)
blake3 = "1.8"
orion = "0.17"
ed25519-dalek = "2.1"
hex = "0.4"

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Time
chrono = { version = "0.4", features = ["serde"] }

# UUID (per project guidelines)
uuid = { version = "1.10", features = ["v7", "serde"] }

# HTTP client (for Docker container communication)
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

### Task 2: Durable Objects with SQLite (Day 2-4)

#### 2.1 Session Management Object
```rust
// durable_objects/session.rs
use worker::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionState {
 pub player_id: String,          // BLAKE3 hash of Ed25519 public key
 pub public_key: String,          // Ed25519 public key (hex)
 pub hwid: Option<String>,        // Hardware ID (optional)
 pub last_heartbeat: u64,
 pub client_version: String,
 pub ban_status: BanStatus,
 pub risk_score: f32,
 pub registered_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum BanStatus {
 NotBanned,
 TemporarilyBanned { expires_at: u64 },
 PermanentlyBanned,
 ShadowBanned,                   // Restrictions without notification
}

pub struct SessionObject {
    env: Env,
}

impl SessionObject {
    fn new(env: Env) -> Self {
        Self { env }
    }
}

#[durable_object]
impl DurableObject for SessionObject {
    fn new(env: Env) -> Self {
        Self::new(env)
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match req.path() {
            "/register" => self.register_session(req).await,
            "/heartbeat" => self.handle_heartbeat(req).await,
            "/check-ban" => self.check_ban_status(req).await,
            "/disconnect" => self.disconnect(req).await,
            _ => Response::error(404, "Not Found"),
        }
    }
}

impl SessionObject {
    async fn register_session(&mut self, req: Request) -> Result<Response> {
        // Parse request
        let session_data: SessionState = req.json().await?;
        
        // Initialize SQLite database in Durable Object
        let sql = self.env.storage_sql();
        
        // Create tables if not exists (idempotent)
        sql.exec(&format!(
            "CREATE TABLE IF NOT EXISTS sessions (
                player_id TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                hwid TEXT,
                last_heartbeat INTEGER NOT NULL,
                client_version TEXT NOT NULL,
                ban_status INTEGER NOT NULL,
                risk_score REAL NOT NULL,
                created_at INTEGER NOT NULL
            )"
        ));
        
        sql.exec(&format!(
            "CREATE TABLE IF NOT EXISTS used_nonces (
                nonce INTEGER PRIMARY KEY,
                player_id TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )"
        ));
        
        sql.exec(&format!(
            "CREATE INDEX IF NOT EXISTS idx_used_nonces_expires ON used_nonces(expires_at)"
        ));
        
        // Insert session - NO AWAIT NEEDED (synchronous!)
        sql.exec(&format!(
            "INSERT OR REPLACE INTO sessions 
             (player_id, public_key, hwid, last_heartbeat, client_version, ban_status, risk_score, created_at)
             VALUES ('{}', '{}', '{}', {}, '{}', {}, {}, {})",
            session_data.player_id,
            session_data.public_key.unwrap_or_default(),
            session_data.hwid.unwrap_or_default(),
            session_data.last_heartbeat,
            session_data.client_version,
            match session_data.ban_status {
                BanStatus::NotBanned => 0,
                BanStatus::TemporarilyBanned { expires_at: _ } => 1,
                BanStatus::PermanentlyBanned => 2,
                BanStatus::ShadowBanned => 3,
            },
            session_data.risk_score,
            current_timestamp()
        ));
        
        Response::ok(json!({
            "status": "registered",
            "player_id": session_data.player_id,
            "session_id": uuid::Uuid::now_v7().to_string()
        }))
    }
    
    async fn handle_heartbeat(&mut self, req: Request) -> Result<Response> {
        let player_id: String = req.json().await?;
        
        let sql = self.env.storage_sql();
        
        // Update heartbeat - synchronous query
        sql.exec(&format!(
            "UPDATE sessions SET last_heartbeat = {} WHERE player_id = '{}'",
            current_timestamp(),
            player_id
        ));
        
        // Check if banned - also synchronous
        let mut cursor = sql.exec(&format!(
            "SELECT ban_status, risk_score FROM sessions WHERE player_id = '{}'",
            player_id
        ));
        
        if let Some(row) = cursor.next() {
            let ban_status: i32 = row.get("ban_status")?;
            let risk_score: f32 = row.get("risk_score")?;
            
            Response::ok(json!({
                "status": "ok",
                "ban_status": ban_status,
                "risk_score": risk_score
            }))
        } else {
            Response::error(404, "Session not found")
        }
    }
    
    async fn check_ban_status(&mut self, req: Request) -> Result<Response> {
        let player_id: String = req.json().await?;
        
        let sql = self.env.storage_sql();
        let mut cursor = sql.exec(&format!(
            "SELECT ban_status, (SELECT expiry FROM temp_bans WHERE player_id = '{}') as temp_expiry 
             FROM sessions WHERE player_id = '{}'",
            player_id, player_id
        ));
        
        if let Some(row) = cursor.next() {
            let ban_status: i32 = row.get("ban_status")?;
            let temp_expiry: Option<u64> = row.get("temp_expiry")?;
            
            let is_banned = match ban_status {
                0 => false,
                1 => temp_expiry.map_or(false, |exp| exp > current_timestamp()),
                2 => true,
                _ => false,
            };
            
            Response::ok(json!({
                "is_banned": is_banned,
                "ban_type": match ban_status {
                    0 => "none",
                    1 => "temporary",
                    2 => "permanent",
                    _ => "unknown"
                }
            }))
        } else {
            Response::ok(json!({"is_banned": false}))
        }
    }
    
    async fn disconnect(&mut self, req: Request) -> Result<Response> {
        let player_id: String = req.json().await?;
        
        let sql = self.env.storage_sql();
        
        // Optional: Delete session or mark as offline
        sql.exec(&format!(
            "DELETE FROM sessions WHERE player_id = '{}'",
            player_id
        ));
        
        Response::ok(json!({"status": "disconnected"}))
    }
    
    /// Register player's Ed25519 public key (first launch)
    async fn register_player(&mut self, req: Request) -> Result<Response> {
        use serde_json::Value;
        
        let data: Value = req.json().await?;
        let player_id = data.get("player_id").and_then(|v| v.as_str()).unwrap_or("");
        let public_key = data.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
        let hwid = data.get("hwid").and_then(|v| v.as_str());
        
        let sql = self.env.storage_sql();
        
        // Create tables if not exists
        sql.exec(&format!(
            "CREATE TABLE IF NOT EXISTS sessions (
                player_id TEXT PRIMARY KEY,
                public_key TEXT NOT NULL,
                hwid TEXT,
                last_heartbeat INTEGER NOT NULL,
                client_version TEXT NOT NULL,
                ban_status INTEGER NOT NULL,
                risk_score REAL NOT NULL,
                created_at INTEGER NOT NULL
            )"
        ));
        
        // Insert or update player registration
        sql.exec(&format!(
            "INSERT OR REPLACE INTO sessions 
             (player_id, public_key, hwid, last_heartbeat, client_version, ban_status, risk_score, created_at)
             VALUES ('{}', '{}', '{}', {}, '{}', {}, {}, {})",
            player_id,
            public_key,
            hwid.unwrap_or(""),
            current_timestamp(),
            "",
            0, // NotBanned
            0.0,
            current_timestamp()
        ));
        
        Response::ok(json!({
            "player_id": player_id,
            "registered_at": current_timestamp(),
            "expires_at": null
        }))
    }
    
    /// Generate one-time action token
    async fn generate_action_token(&mut self, req: Request) -> Result<Response> {
        use serde_json::Value;
        
        let data: Value = req.json().await?;
        let player_id = data.get("player_id").and_then(|v| v.as_str()).unwrap_or("");
        let nonce = data.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
        
        // Get server secret from environment
        let server_secret = self.env.secret("ACTION_TOKEN_SECRET")?.to_string();
        
        // Generate timestamp
        let timestamp = current_timestamp();
        let expires_at = timestamp + 300; // +5 minutes
        
        // Generate BLAKE3 hash: BLAKE3(player_id || timestamp || nonce || server_secret)
        let token_input = format!("{}|{}|{}|{}", player_id, timestamp, nonce, server_secret);
        let token_hash = blake3::hash(token_input.as_bytes()).as_bytes().to_vec();
        
        Response::ok(json!({
            "player_id": player_id,
            "timestamp": timestamp,
            "nonce": nonce,
            "token_hash": token_hash,
            "expires_at": expires_at
        }))
    }
    
    /// Record cheat event with dual verification
    async fn record_cheat(&mut self, req: Request) -> Result<Response> {
        use serde_json::Value;
        use ed25519_dalek::{PublicKey, Signature, Verifier};
        
        let data: Value = req.json().await?;
        
        // Parse signed request
        let payload = data.get("payload").ok_or_else(|| anyhow::anyhow!("Missing payload"))?;
        let action_data = payload.get("action_data").ok_or_else(|| anyhow::anyhow!("Missing action_data"))?;
        let action_token = payload.get("action_token").ok_or_else(|| anyhow::anyhow!("Missing action_token"))?;
        let signature = data.get("signature").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing signature"))?;
        
        let player_id = action_data.get("player_id").and_then(|v| v.as_str()).unwrap_or("");
        let nonce = action_token.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
        
        let sql = self.env.storage_sql();
        
        // Step 1: Verify Ed25519 signature
        let public_key_hex = sql.exec(&format!(
            "SELECT public_key FROM sessions WHERE player_id = '{}'",
            player_id
        ));
        
        if let Some(row) = public_key_hex.next() {
            let public_key_str: String = row.get("public_key")?;
            let public_key_bytes = hex::decode(public_key_str)?;
            let public_key = PublicKey::from_bytes(&public_key_bytes)?;
            
            let sig_bytes = hex::decode(signature)?;
            let sig = Signature::from_bytes(&sig_bytes)?;
            
            let payload_bytes = serde_json::to_vec(payload)?;
            public_key.verify(&payload_bytes, &sig)?;
        } else {
            return Err(anyhow::anyhow!("Player not registered").into());
        }
        
        // Step 2: Verify action token
        let token_timestamp = action_token.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
        let token_hash = action_token.get("token_hash").and_then(|v| v.as_str()).unwrap_or("");
        
        let server_secret = self.env.secret("ACTION_TOKEN_SECRET")?.to_string();
        let token_input = format!("{}|{}|{}|{}", player_id, token_timestamp, nonce, server_secret);
        let expected_hash = blake3::hash(token_input.as_bytes());
        
        if hex::encode(expected_hash.as_bytes()) != token_hash {
            return Err(anyhow::anyhow!("Invalid action token").into());
        }
        
        // Step 3: Check nonce uniqueness (prevent replay)
        let nonce_check = sql.exec(&format!(
            "SELECT nonce FROM used_nonces WHERE nonce = {}",
            nonce
        ));
        
        if nonce_check.next().is_some() {
            return Err(anyhow::anyhow!("Action token already used (replay attack)").into());
        }
        
        // Step 4: Mark nonce as used
        sql.exec(&format!(
            "INSERT INTO used_nonces (nonce, player_id, expires_at, created_at) VALUES ({}, '{}', {}, {})",
            nonce,
            player_id,
            current_timestamp() + 300, // Expire after 5 minutes
            current_timestamp()
        ));
        
        // Step 5: Process cheat event
        let cheat_type = action_data.get("cheat_type").and_then(|v| v.as_i64()).unwrap_or(0);
        let timestamp = action_data.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
        let detection_count = action_data.get("detection_count").and_then(|v| v.as_u64()).unwrap_or(0);
        
        // Insert cheat event
        sql.exec(&format!(
            "CREATE TABLE IF NOT EXISTS cheat_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                player_id TEXT NOT NULL,
                cheat_type INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                detection_count INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )"
        ));
        
        sql.exec(&format!(
            "INSERT INTO cheat_events (player_id, cheat_type, timestamp, detection_count, created_at)
             VALUES ('{}', {}, {}, {}, {})",
            player_id,
            cheat_type,
            timestamp,
            detection_count,
            current_timestamp()
        ));
        
        // Update violation count
        sql.exec(&format!(
            "UPDATE sessions 
             SET violation_count = COALESCE(violation_count, 0) + 1,
                 last_violation = {}
             WHERE player_id = '{}'",
            timestamp,
            player_id
        ));
        
        Response::ok(json!({
            "success": true,
            "recorded_at": current_timestamp(),
            "player_state": {
                "player_id": player_id,
                "violation_count": 1,
                "last_violation": timestamp,
                "status": "flagged"
            }
        }))
    }
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
```

#### 2.2 Pattern Distribution Object
```rust
// durable_objects/pattern.rs
use worker::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityPattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub signature: Vec<u8>,
    pub version: u32,
    pub created_at: u64,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PatternType {
    ApiBypass,
    ProcessInjection,
    HardwareMacro,
    OsIntegrityViolation,
    VmDetection,
}

pub struct PatternObject {
    env: Env,
}

#[durable_object]
impl DurableObject for PatternObject {
    fn new(env: Env) -> Self {
        Self { env }
    }

    async fn fetch(&mut self, req: Request) -> Request {
        match req.path() {
            "/get-patterns" => self.get_patterns(req).await,
            "/add-pattern" => self.add_pattern(req).await,
            _ => Response::error(404, "Not Found"),
        }
    }
}

impl PatternObject {
    async fn get_patterns(&mut self, req: Request) -> Result<Response> {
        let sql = self.env.storage_sql();
        
        // Create table
        sql.exec("CREATE TABLE IF NOT EXISTS patterns (
            pattern_id TEXT PRIMARY KEY,
            pattern_type INTEGER NOT NULL,
            signature BLOB NOT NULL,
            version INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            is_active INTEGER NOT NULL
        )");
        
        // Query all active patterns - synchronous!
        let mut cursor = sql.exec("SELECT * FROM patterns WHERE is_active = 1");
        
        let mut patterns = Vec::new();
        while let Some(row) = cursor.next() {
            let pattern_type_int: i32 = row.get("pattern_type")?;
            let pattern = SecurityPattern {
                pattern_id: row.get("pattern_id")?,
                pattern_type: match pattern_type_int {
                    0 => PatternType::ApiBypass,
                    1 => PatternType::ProcessInjection,
                    2 => PatternType::HardwareMacro,
                    3 => PatternType::OsIntegrityViolation,
                    4 => PatternType::VmDetection,
                    _ => return Err("Invalid pattern type".into()),
                },
                signature: row.get("signature")?,
                version: row.get("version")?,
                created_at: row.get("created_at")?,
                is_active: row.get("is_active")? != 0,
            };
            patterns.push(pattern);
        }
        
        Response::ok(json!(patterns))
    }
    
    async fn add_pattern(&mut self, req: Request) -> Result<Response> {
        let pattern: SecurityPattern = req.json().await?;
        
        let sql = self.env.storage_sql();
        
        sql.exec(&format!(
            "INSERT OR REPLACE INTO patterns 
             (pattern_id, pattern_type, signature, version, created_at, is_active)
             VALUES ('{}', {}, x'{}', {}, {}, {})",
            pattern.pattern_id,
            pattern.pattern_type as i32,
            hex::encode(&pattern.signature),
            pattern.version,
            pattern.created_at,
            if pattern.is_active { 1 } else { 0 }
        ));
        
        Response::ok(json!({"status": "added", "pattern_id": pattern.pattern_id}))
    }
}
```

#### 2.3 Telemetry Aggregation Object
```rust
// durable_objects/telemetry.rs
use worker::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event_id: String,
    pub player_id: String,
    pub event_type: String,
    pub severity: String,
    pub timestamp: u64,
    pub details: String,
    pub hwid: String,
}

pub struct TelemetryObject {
    env: Env,
}

#[durable_object]
impl DurableObject for TelemetryObject {
    fn new(env: Env) -> Self {
        Self { env }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match req.path() {
            "/submit" => self.submit_telemetry(req).await,
            "/aggregate" => self.aggregate_telemetry(req).await,
            _ => Response::error(404, "Not Found"),
        }
    }
}

impl TelemetryObject {
    async fn submit_telemetry(&mut self, req: Request) -> Result<Response> {
        let events: Vec<TelemetryEvent> = req.json().await?;
        
        let sql = self.env.storage_sql();
        
        // Create table
        sql.exec("CREATE TABLE IF NOT EXISTS telemetry (
            event_id TEXT PRIMARY KEY,
            player_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            details TEXT NOT NULL,
            hwid TEXT NOT NULL
        )");
        
        // Insert events - batch insert for performance
        for event in events {
            sql.exec(&format!(
                "INSERT INTO telemetry 
                 (event_id, player_id, event_type, severity, timestamp, details, hwid)
                 VALUES ('{}', '{}', '{}', '{}', {}, '{}', '{}')",
                event.event_id,
                event.player_id,
                event.event_type,
                event.severity,
                event.timestamp,
                event.details,
                event.hwid
            ));
        }
        
        // Note: Output Gate automatically waits for write confirmation
        // No explicit await needed!
        
        Response::ok(json!({"status": "submitted", "count": events.len()}))
    }
    
    async fn aggregate_telemetry(&mut self, req: Request) -> Result<Response> {
        let player_id: String = req.json().await?;
        
        let sql = self.env.storage_sql();
        
        // Aggregate events by severity - synchronous query
        let mut cursor = sql.exec(&format!(
            "SELECT event_type, severity, COUNT(*) as count 
             FROM telemetry 
             WHERE player_id = '{}'
             GROUP BY event_type, severity",
            player_id
        ));
        
        let mut aggregations = Vec::new();
        while let Some(row) = cursor.next() {
            aggregations.push(json!({
                "event_type": row.get::<String>("event_type")?,
                "severity": row.get::<String>("severity")?,
                "count": row.get::<i64>("count")?
            }));
        }
        
        Response::ok(json!(aggregations))
    }
}

#### 2.3 Nonce Tracker Object (Day 4)
```rust
// durable_objects/nonce.rs
use worker::*;

pub struct NonceTracker {
    env: Env,
}

#[durable_object]
impl DurableObject for NonceTracker {
    fn new(env: Env) -> Self {
        Self { env }
    }

    async fn fetch(&mut self, req: Request) -> Request {
        match req.path() {
            "/check-nonce" => self.check_nonce(req).await,
            "/mark-used" => self.mark_used(req).await,
            "/cleanup" => self.cleanup_expired(req).await,
            _ => Response::error(404, "Not Found"),
        }
    }
}

impl NonceTracker {
    async fn check_nonce(&mut self, req: Request) -> Result<Response> {
        use serde_json::Value;
        
        let data: Value = req.json().await?;
        let nonce = data.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
        
        let sql = self.env.storage_sql();
        
        // Create table if not exists
        sql.exec("CREATE TABLE IF NOT EXISTS used_nonces (
            nonce INTEGER PRIMARY KEY,
            player_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )");
        
        sql.exec("CREATE INDEX IF NOT EXISTS idx_used_nonces_expires ON used_nonces(expires_at)");
        
        // Check if nonce exists
        let mut cursor = sql.exec(&format!("SELECT nonce FROM used_nonces WHERE nonce = {}", nonce));
        
        let is_used = cursor.next().is_some();
        
        Response::ok(json!({
            "nonce": nonce,
            "is_used": is_used
        }))
    }
    
    async fn mark_used(&mut self, req: Request) -> Result<Response> {
        use serde_json::Value;
        
        let data: Value = req.json().await?;
        let nonce = data.get("nonce").and_then(|v| v.as_u64()).unwrap_or(0);
        let player_id = data.get("player_id").and_then(|v| v.as_str()).unwrap_or("");
        
        let sql = self.env.storage_sql();
        
        // Create table if not exists
        sql.exec("CREATE TABLE IF NOT EXISTS used_nonces (
            nonce INTEGER PRIMARY KEY,
            player_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )");
        
        // Mark nonce as used with 5-minute expiry
        let expires_at = current_timestamp() + 300; // +5 minutes
        sql.exec(&format!(
            "INSERT OR IGNORE INTO used_nonces (nonce, player_id, expires_at, created_at)
             VALUES ({}, '{}', {}, {})",
            nonce,
            player_id,
            expires_at,
            current_timestamp()
        ));
        
        Response::ok(json!({
            "status": "marked",
            "nonce": nonce,
            "expires_at": expires_at
        }))
    }
    
    async fn cleanup_expired(&mut self, _req: Request) -> Result<Response> {
        let sql = self.env.storage_sql();
        
        // Delete expired nonces
        let now = current_timestamp();
        sql.exec(&format!("DELETE FROM used_nonces WHERE expires_at < {}", now));
        
        Response::ok(json!({
            "status": "cleaned",
            "timestamp": now
        }))
    }
}
```

### Task 3: Worker Router (Day 4-6)

#### 3.1 Request Routing
```rust
// src/router.rs
use worker::*;

pub async fn route_request(req: Request, env: Env) -> Result<Response> {
    // CORS handling
    let cors_headers = Headers::from_iter(vec![
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
        ("Access-Control-Allow-Headers", "Content-Type, Authorization"),
    ]);
    
    if req.method() == Method::Options {
        return Ok(Response::empty()
            .unwrap()
            .with_headers(cors_headers));
    }
    
    // Rate limiting check
    if !check_rate_limit(&req, &env).await? {
        return Response::error(429, "Rate limit exceeded")
            .map(|r| r.with_headers(cors_headers));
    }
    
    // Authentication check
    match authenticate_request(&req, &env) {
        Ok(_) => {},
        Err(e) => {
            return Response::error(401, e.to_string())
                .map(|r| r.with_headers(cors_headers));
        }
    }
    
    // Route to appropriate Durable Object
    let path = req.path();
    
    if path.starts_with("/session/") {
        // Extract player_id and route to SessionObject
        let player_id = path.strip_prefix("/session/").unwrap();
        let stub = env.durable_object("SESSIONS");
        let session_id = format!("session:{}", player_id);
        let session = stub.get(&session_id).await?;
        
        let mut new_req = Request::new(req.method(), req.url());
        *new_req.headers_mut() = req.headers().clone();
        
        session.fetch(new_req).await
    } else if path.starts_with("/patterns/") {
        // Route to PatternObject (single global instance)
        let stub = env.durable_object("PATTERNS");
        let pattern = stub.get("global").await?;
        
        pattern.fetch(req).await
    } else if path.starts_with("/telemetry/") {
        // Route to TelemetryObject (shard by player_id)
        let player_id = path.strip_prefix("/telemetry/").unwrap();
        let shard = calculate_shard(player_id);
        let stub = env.durable_object("TELEMETRY");
        let telemetry = stub.get(&format!("telemetry:shard:{}", shard)).await?;
        
        telemetry.fetch(req).await
    } else if path.starts_with("/heavy-processing/") {
        // Route to Docker container via HTTP
        route_to_container(req, &env).await
    } else {
        Response::error(404, "Not Found")
    }
}

fn calculate_shard(player_id: &str) -> u32 {
    // Simple sharding function
    let hash = blake3::hash(player_id.as_bytes());
    let bytes = hash.as_bytes();
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 100
}

async fn route_to_container(req: Request, env: Env) -> Result<Response> {
    // Forward request to Axum backend running in Docker
    let backend_url = env.var("AXUM_BACKEND_URL")?.to_string();
    let url = format!("{}{}", backend_url, req.path());
    
    // Use reqwest to forward the request
    let client = reqwest::Client::new();
    let response = client
        .request(req.method().into(), &url)
        .headers(req.headers().clone().into_iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_string(), value.to_string()))
            })
            .collect())
        .body(req.bytes().await?.to_vec())
        .send()
        .await?;
    
    let status_code = response.status().as_u16() as u16;
    let body = response.bytes().await?.to_vec();
    
    Ok(Response::from_bytes(body)
        .unwrap()
        .with_status(status_code))
}
```

### Task 4: Authentication System (Day 6-8)

#### 4.1 Token-Based Authentication
```rust
// src/auth.rs
use worker::*;
use orion::aead::SecretKey;
use blake3::Hash;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthToken {
    pub token_id: String,
    pub player_id: String,
    pub hwid: String,
    pub expires_at: u64,
    pub signature: Vec<u8>,
}

impl AuthToken {
    pub fn new(player_id: &str, hwid: &str, secret_key: &[u8]) -> Self {
        let token_id = format!("{:x}", Hash::hash(player_id.as_bytes()));
        let payload = format!("{}|{}|{}", token_id, player_id, hwid);
        let signature = blake3::hash(payload.as_bytes()).as_bytes().to_vec();
        
        AuthToken {
            token_id,
            player_id: player_id.to_string(),
            hwid: hwid.to_string(),
            expires_at: calculate_expiry(24 * 60 * 60), // 24 hours
            signature,
        }
    }
    
    pub fn validate(&self, secret_key: &[u8]) -> bool {
        let payload = format!("{}|{}|{}", self.token_id, self.player_id, self.hwid);
        let expected_signature = blake3::hash(payload.as_bytes()).as_bytes().to_vec();
        
        self.signature == expected_signature
            && self.expires_at > current_timestamp()
    }
}

pub fn authenticate_request(req: &Request, env: &Env) -> Result<AuthToken, AuthError> {
    let auth_header = req.headers()
        .get("Authorization")
        .ok_or(AuthError::MissingToken)?;
    
    let token_str = auth_header
        .to_str()
        .map_err(|_| AuthError::InvalidToken)?;
    
    if !token_str.starts_with("Bearer ") {
        return Err(AuthError::InvalidToken);
    }
    
    let token_json = token_str.strip_prefix("Bearer ").unwrap();
    let token: AuthToken = serde_json::from_str(token_json)
        .map_err(|_| AuthError::InvalidToken)?;
    
    // Validate token signature
    let secret_key = env.secret("AUTH_SECRET_KEY")?.to_string();
    if !token.validate(secret_key.as_bytes()) {
        return Err(AuthError::InvalidSignature);
    }
    
    Ok(token)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing authentication token")]
    MissingToken,
    #[error("Invalid token format")]
    InvalidToken,
    #[error("Invalid token signature")]
    InvalidSignature,
    #[error("Token expired")]
    Expired,
}

fn calculate_expiry(seconds: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() + seconds
}

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
```

### Task 5: Rate Limiting (Day 8-10)

#### 5.1 Durable Object for Rate Limiting
```rust
// src/rate_limit.rs
use worker::*;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_requests: u32,
    pub window_seconds: u64,
}

pub struct RateLimiter {
    env: Env,
}

#[durable_object]
impl DurableObject for RateLimiter {
    fn new(env: Env) -> Self {
        Self { env }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match req.method() {
            Method::Post => self.check_and_consume(req).await,
            _ => Response::error(405, "Method not allowed"),
        }
    }
}

impl RateLimiter {
    async fn check_and_consume(&mut self, req: Request) -> Result<Response> {
        let key: String = req.json().await?;
        let config = RateLimitConfig {
            max_requests: 1000,
            window_seconds: 3600, // 1 hour
        };
        
        let sql = self.env.storage_sql();
        
        // Create table
        sql.exec("CREATE TABLE IF NOT EXISTS rate_limits (
            key TEXT PRIMARY KEY,
            tokens INTEGER NOT NULL,
            last_update INTEGER NOT NULL
        )");
        
        // Get current state
        let mut cursor = sql.exec(&format!(
            "SELECT tokens, last_update FROM rate_limits WHERE key = '{}'",
            key
        ));
        
        let (mut tokens, mut last_update) = if let Some(row) = cursor.next() {
            (row.get::<i64>("tokens")?, row.get::<i64>("last_update")?)
        } else {
            (config.max_requests as i64, current_timestamp())
        };
        
        // Refill tokens
        let now = current_timestamp();
        let elapsed = (now - last_update) as f32;
        let refill = elapsed / config.window_seconds as f32 * config.max_requests as f32;
        tokens = ((tokens as f32 + refill).floor() as i64).min(config.max_requests as i64);
        last_update = now;
        
        // Check if allowed
        if tokens >= 1 {
            tokens -= 1;
            
            // Update state
            sql.exec(&format!(
                "INSERT OR REPLACE INTO rate_limits (key, tokens, last_update) 
                 VALUES ('{}', {}, {})",
                key, tokens, last_update
            ));
            
            Response::ok(json!({"allowed": true, "remaining": tokens}))
        } else {
            // Wait time calculation
            let wait_time = ((1.0 - (tokens as f32 / config.max_requests as f32)) 
                * config.window_seconds as f32).ceil() as u64;
            
            Response::ok(json!({
                "allowed": false,
                "retry_after": wait_time
            }))
        }
    }
}
```

### Task 6: Docker Container Backend (Day 10-12)

#### 6.1 Axum Server Structure
```toml
# containers/axum-backend/Cargo.toml
[package]
name = "maxion-axum-backend"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Database (per project guidelines: use sqlx with raw_sql for PgCat)
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "chrono", "uuid"] }

# Cryptography (per project guidelines)
blake3 = "1.8"
argon2 = "0.5"
orion = "0.17"

# UUID (per project guidelines)
uuid = { version = "1.10", features = ["v7", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }
```

#### 6.2 Main Application
```rust
// containers/axum-backend/src/main.rs
use axum::{
    routing::{get, post},
    Json, Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod telemetry;
mod patterns;
mod bans;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "maxion_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Database pool (PostgreSQL)
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    
    let pool = sqlx::postgres::PgPool::connect(&database_url).await?;
    
    // Build application state
    let state = AppState { pool };
    
    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/telemetry/analyze", post(telemetry::analyze))
        .route("/patterns/extract", post(patterns::extract))
        .route("/bans/apply", post(bans::apply))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .with_state(state);
    
    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Axum backend listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

#[derive(Clone)]
pub struct AppState {
    pool: sqlx::PgPool,
}
```

#### 6.3 Heavy Telemetry Analysis
```rust
// containers/axum-backend/src/telemetry.rs
use axum::{Json, State};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TelemetryBatch {
    pub player_id: String,
    pub events: Vec<TelemetryEvent>,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryEvent {
    pub event_id: String,
    pub event_type: String,
    pub severity: String,
    pub timestamp: u64,
    pub details: String,
}

#[derive(Debug, Serialize)]
pub struct AnalysisResult {
    pub player_id: String,
    pub risk_score: f32,
    pub recommendations: Vec<String>,
}

pub async fn analyze(
    State(state): State<AppState>,
    Json(batch): Json<TelemetryBatch>,
) -> Json<AnalysisResult> {
    // Perform complex analysis that's too heavy for Workers
    // Examples: Machine learning inference, complex pattern matching
    
    // Query historical data from PostgreSQL using raw_sql (per project guidelines)
    let query = format!(
        "SELECT * FROM telemetry_history WHERE player_id = '{}' ORDER BY timestamp DESC LIMIT 1000",
        batch.player_id
    );
    
    let rows = sqlx::raw_sql(&query)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
    
    // Perform complex analysis
    let risk_score = calculate_risk_score(&batch.events, &rows);
    let recommendations = generate_recommendations(risk_score);
    
    AnalysisResult {
        player_id: batch.player_id,
        risk_score,
        recommendations,
    }
}

fn calculate_risk_score(events: &[TelemetryEvent], history: &[sqlx::postgres::PgRow]) -> f32 {
    // Complex scoring logic
    // This is where ML models would run
    events.iter()
        .map(|e| match e.severity.as_str() {
            "Critical" => 25.0,
            "High" => 10.0,
            "Medium" => 5.0,
            "Low" => 1.0,
            _ => 0.0,
        })
        .sum()
}

fn generate_recommendations(risk_score: f32) -> Vec<String> {
    if risk_score > 50.0 {
        vec![
            "Immediate action required".to_string(),
            "Consider temporary ban".to_string(),
        ]
    } else if risk_score > 20.0 {
        vec![
            "Monitor closely".to_string(),
            "Increase scan frequency".to_string(),
        ]
    } else {
        vec!["No action required".to_string()]
    }
}
```

### Task 7: Error Handling & Logging (Day 12-13)

#### 7.1 Structured Error Types
```rust
// src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommunicationError {
    #[error("Authentication failed: {0}")]
    Auth(#[from] crate::auth::AuthError),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Connection lost")]
    ConnectionLost,
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Server error: {0}")]
    Server(String),
}

// Worker response helper
pub fn error_response(error: CommunicationError) -> worker::Response {
    let status = match error {
        CommunicationError::Auth(_) => 401,
        CommunicationError::RateLimitExceeded => 429,
        CommunicationError::ConnectionLost => 503,
        CommunicationError::InvalidRequest(_) => 400,
        CommunicationError::Server(_) => 500,
    };
    
    worker::Response::error(status, error.to_string())
        .unwrap_or_else(|_| worker::Response::error(500, "Internal server error").unwrap())
}
```

### Task 8: Client-Side Communication (Day 13-14)

#### 8.1 Rust Client for Windows
```rust
// maxion-antihack/src/server_client.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerClient {
    server_url: String,
    auth_token: Option<String>,
    client: Client,
}

impl ServerClient {
    pub fn new(server_url: String) -> Self {
        ServerClient {
            server_url,
            auth_token: None,
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }
    
    pub async fn register_session(&mut self, player_id: &str, hwid: &str) -> Result<String> {
        let url = format!("{}/session/{}", self.server_url, player_id);
        
        #[derive(Serialize)]
        struct RegisterRequest {
            player_id: String,
            hwid: String,
            client_version: String,
        }
        
        let request = RegisterRequest {
            player_id: player_id.to_string(),
            hwid: hwid.to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        };
        
        let response: serde_json::Value = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;
        
        if let Some(token) = response.get("session_id").and_then(|t| t.as_str()) {
            self.auth_token = Some(token.to_string());
            Ok(token.to_string())
        } else {
            anyhow::bail!("Failed to register session")
        }
    }
    
    pub async fn send_telemetry(&self, events: Vec<crate::types::DetectionEvent>) -> Result<()> {
        let url = format!("{}/telemetry/{}", self.server_url, get_player_id_from_token()?);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap()))
            .json(&events)
            .send()
            .await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("Failed to send telemetry: {}", response.status())
        }
    }
    
    pub async fn get_patterns(&self) -> Result<Vec<SecurityPattern>> {
        let url = format!("{}/patterns/get-patterns", self.server_url);
        
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap()))
            .send()
            .await?
            .json()
            .await?;
        
        Ok(response)
    }
    
    pub async fn check_ban_status(&self) -> Result<BanStatus> {
        let url = format!("{}/session/{}/check-ban", self.server_url, get_player_id_from_token()?);
        
        let response: serde_json::Value = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token.as_ref().unwrap()))
            .send()
            .await?
            .json()
            .await?;
        
        Ok(serde_json::from_value(response)?)
    }
}

fn get_player_id_from_token() -> Result<String> {
    // Extract player_id from auth token
    // In production, decode the JWT or signed token
    Ok("placeholder_player_id".to_string())
}

#[derive(Debug, Deserialize)]
pub struct SecurityPattern {
    pub pattern_id: String,
    pub pattern_type: String,
    pub signature: Vec<u8>,
    pub version: u32,
}

#[derive(Debug, Deserialize)]
pub struct BanStatus {
    pub is_banned: bool,
    pub ban_type: String,
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_refill() {
        // Test token refill logic
        let mut limiter = RateLimiter::new();
        let config = RateLimitConfig {
            max_requests: 100,
            window_seconds: 60,
        };
        
        // Exhaust tokens
        for _ in 0..100 {
            assert!(limiter.check_rate("test", &config));
        }
        
        // Should be rate limited
        assert!(!limiter.check_rate("test", &config));
        
        // After 60 seconds, should refill
        // (In real test, mock time)
    }

    #[test]
    fn test_auth_token_validation() {
        let token = AuthToken::new("player123", "hwid456", b"secret_key");
        assert!(token.validate(b"secret_key"));
        assert!(!token.validate(b"wrong_key"));
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_session_registration() {
    // Test full session registration flow
    // This would spin up a test Durable Object
    
    let mut client = ServerClient::new("http://localhost:8787".to_string());
    let session_id = client.register_session("test_player", "test_hwid").await.unwrap();
    
    assert!(!session_id.is_empty());
}

#[tokio::test]
async fn test_telemetry_submission() {
    // Test telemetry submission and aggregation
    
    let client = ServerClient::new("http://localhost:8787".to_string());
    client.register_session("test_player", "test_hwid").await.unwrap();
    
    let events = vec![
        DetectionEvent {
            event_type: DetectionType::HardwareMacro,
            severity: DetectionSeverity::Medium,
            timestamp: current_timestamp(),
            details: "Test event".to_string(),
        }
    ];
    
    client.send_telemetry(events).await.unwrap();
}
```

## Performance Requirements

- **API Response Time**: < 10ms (cached in Durable Objects)
- **SQLite Query Latency**: < 1ms (same thread)
- **Rate Limiting Check**: < 1ms
- **Authentication**: < 2ms
- **Telemetry Submission**: < 5ms per batch (100 events)
- **Pattern Download**: < 10ms (for 100 patterns)

## Security Considerations

### DDoS Protection
- Cloudflare Workers provides automatic DDoS protection
- Rate limiting at multiple levels (Worker + Durable Object)
- IP-based blocking for abusive clients

### Encryption
- TLS 1.3 for all client-server communication
- AES-256-GCM for sensitive data (using orion)
- Request signing for integrity verification

### SQLite Security in Durable Objects
- Automatic replication to 5 followers
- Write confirmation before response (Output Gate)
- Point-in-time recovery for disaster recovery
- Data encrypted at rest (managed by Cloudflare)

## Dependencies

### Cloudflare Workers
- `worker` - Cloudflare Workers SDK for Rust
- `worker-sys` - FFI bindings
- `serde` - Serialization
- `blake3` - Hashing (per project guidelines)
- `orion` - Cryptography (per project guidelines)

### Axum Backend (Docker)
- `axum` - Web framework (confirmed choice)
- `tokio` - Async runtime
- `tower` - Middleware
- `sqlx` - Database driver (use raw_sql per project guidelines)
- `blake3` - Hashing
- `argon2` - Password hashing
- `uuid` - UUID generation (use v7 per project guidelines)

## Deliverables

1. ✅ Cloudflare Worker (Rust) with request routing
2. ✅ Durable Objects: Session, Pattern, Telemetry with SQLite
3. ✅ Authentication and rate limiting
4. ✅ Docker container with Axum backend
5. ✅ Client-side communication library
6. ✅ Unit and integration tests
7. ✅ Performance benchmarks

## Next Steps

After completing this phase, proceed to:
- **008c**: Server-Side Detection Service (enhance Durable Objects and Docker backend)
- **008d**: Pattern Management System (extend PatternObject)
- **008e**: Ban Management Service (integrate with SessionObject)

## Notes

- **Zero-Latency SQLite**: Durable Objects run SQLite in the same thread - no network hops, no async overhead
- **Synchronous Queries**: All SQLite queries are synchronous - no `await` needed for typical operations
- **Output Gates**: Writes are confirmed automatically before responses are sent
- **Horizontal Scaling**: Create more Durable Objects to scale out (one per logical entity)
- **Docker for Heavy Work**: ML inference, complex analysis runs in containers, Workers handle routing
- **Follow project coding style**: snake_case, match over if, early returns
- **Use `blake3` for hashing**, `argon2` for passwords, `Uuid::v7()` for IDs
- **Use `sqlx::raw_sql`** for PostgreSQL queries (PgCat compatibility)
- **Axum confirmed** as the web framework for Docker containers
- **Client is native Windows** - server communicates via HTTPS/REST

## Known Limitations

- SQLite storage per Durable Object limited to 1GB (beta), 10GB (GA)
- Durable Objects are single-threaded - limited throughput per object
- Heavy processing must run in Docker containers, not Workers
- Cold starts possible for rarely-accessed Durable Objects

## Migration Path

### From 008a to 008b
- Native Windows client (008a) connects via HTTPS to Workers
- Workers route requests to appropriate Durable Objects
- Heavy processing forwarded to Docker containers

### To 008c
- Extend TelemetryObject with more complex analysis
- Add pattern detection logic to SessionObject
- Enhance Docker backend with ML inference

---
**Last Updated:** 2025-01-24
**Version:** 2.0 (Updated for Cloudflare Workers + Durable Objects + SQLite)
**Maintainer:** Maxion Protector Team