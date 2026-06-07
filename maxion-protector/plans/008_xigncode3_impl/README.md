# XIGNCODE3 Lightweight Sidecar Implementation Plan

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-21 |
| Version | 2.0 (Lightweight Sidecar) |
| Complexity | Advanced |
| Time to Read | 30 minutes |
| Audience | Architects, Senior Developers, Security Engineers |

## Overview

This plan describes a **lightweight sidecar service** for anti-cheat detection and event marking. Unlike traditional end-to-end anti-cheat systems, this service focuses solely on:

1. **Detecting** cheat events from client traps
2. **Marking** player UUID states in KV
3. **Providing** query endpoints for other systems to judge

This system **does not**:
- Handle authentication (use existing auth server)
- Manage bans (let other systems decide)
- Store patterns (simplified detection only)
- Run analytics (use existing monitoring)

**Architecture Pattern:** Sidecar / Event Store
**Primary Goal:** Fast, reliable cheat event recording with nanosecond timestamps
**Secondary Goal:** Queryable state for other systems to act upon

## Documentation Structure

This implementation plan is organized into the following documents:

- **[README.md](./README.md)** - This file. Overview and high-level plan
- **[ARCHITECTURE_SUMMARY.md](./ARCHITECTURE_SUMMARY.md)** - Detailed architecture diagrams, component breakdown, data flows, and system design
- **[TASKS.md](./TASKS.md)** - Complete implementation checklist with phases, acceptance criteria, testing requirements, and deployment procedures

### Shared Foundation Documents

These documents describe the shared foundation for the anti-cheat system:
- **[000_shared_types.md](./000_shared_types.md)** - Shared types crate specification (`maxion-detection-types`)
- **[001_storage_abstraction.md](./001_storage_abstraction.md)** - Storage abstraction layer specification (`maxion-detection-core`)
- **[002_client_consolidation.md](./002_client_consolidation.md)** - Client communication consolidation specification (`maxion-anticheat-client`)

### Detailed Phase Documents

Each phase has its own detailed implementation guide:
- **[008a_client_foundation.md](./008a_client_foundation.md)** - Native Windows client detection engine
- **[008b_client_server_comm.md](./008b_client_server_comm.md)** - Cloudflare Workers and communication layer
- **[008c_server_detection.md](./008c_server_detection.md)** - Server-side detection logic
- **[008d_pattern_management.md](./008d_pattern_management.md)** - Security pattern distribution
- **[008e_ban_management.md](./008e_ban_management.md)** - Ban enforcement and management
- **[008f_analytics_monitoring.md](./008f_analytics_monitoring.md)** - Analytics and monitoring system

### Architecture Changes

**Shared Foundation:**
- All phases use shared `maxion-detection-types` crate for type definitions
- All phases use `maxion-detection-core` storage abstraction layer
- Client communication unified in `maxion-anticheat-client` crate
- Eliminated type duplication across phases
- Reduced coupling to Cloudflare infrastructure

## Architecture Overview

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────┐
│                   Game Client (Unity + Rust)                │
│  • Protected<T> values with trap detection (006_trap.md)   │
│  • UUID v7 for player identification                        │
│  • Callback system for cheat detection                      │
└─────────────────────┬───────────────────────────────────────┘
                      │ POST /cheat (cheat event)
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              Cloudflare Worker (Axum)                       │
│  • Lightweight request routing                              │
│  • Request validation (UUID format, timestamp bounds)       │
│  • Routes to Durable Object based on player_uuid            │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              Durable Object (per player UUID)                │
│  • SQLite for player-specific cheat history                 │
│  • Manages state: recent events, violation counts           │
│  • Writes to KV with smart key naming                       │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│               KV (Key-Value Store)                          │
│  • Key format: ${game_id}|${version}|${player_uuid}        │
│  • Stores cheat events with nanosecond timestamps           │
│  • Supports prefix filtering for queries                    │
│  • Other systems consume from here                          │
└─────────────────────────────────────────────────────────────┘

                      ▲
                      │ (optional)
                      ▼
┌─────────────────────────────────────────────────────────────┐
│            Docker Containers (Heavy Processing)             │
│  • Complex cheat detection algorithms                       │
│  • ML-based pattern analysis                                │
│  • Called from DO if needed                                 │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

#### 1. Client Side (Refactored)
- **Reference:** `002_client_consolidation.md`
- **Crate:** `maxion-anticheat-client`
- **Features:**
  - Unified client communication (consolidated from 008a + 008b)
  - Ed25519 key pair generation and management
  - BLAKE3-based player ID derivation
  - Action token request and management
  - Cheat event submission with dual verification
  - Ban status checking
  - Pattern fetching
  - Retry logic with exponential backoff
  - Offline support with local buffering
  - Clean FFI interface for Unity integration

**Unity Integration:**
- Single native library: `maxion_anticheat_client`
- FFI exports: `maxion_anticheat_init`, `maxion_get_player_id`, `maxion_submit_cheat_event`, `maxion_check_ban_status`
- All types from shared `maxion-detection-types` crate
  - Payload signing

**Callback Signature:**
```rust
extern "C" fn cheat_callback(
    cheat_type: i32,           // CheatType enum value
    hwid_ptr: *const u8,       // Hardware ID bytes
    hwid_len: usize,           // HWID length
    timestamp: u64,            // Nanoseconds since epoch
    detection_count: u32,      // How many times detected
)
```

**Enhanced Integration Flow with Action Tokens:**
```rust
// In game client (FFI from Unity)
use maxion_core::Protected;
use uuid::Uuid;
use ed25519_dalek::{Keypair, Signer};
use blake3;

// Client-side state (persisted securely)
static CLIENT_KEYPAIR: Lazy<Keypair> = Lazy::new(|| {
    // Load or generate Ed25519 key pair at first launch
    load_or_generate_keypair()
});

// Derive player_id from public key using BLAKE3
fn derive_player_id(public_key: &PublicKey) -> String {
    format!("{:x}", blake3::hash(public_key.as_bytes()))
}

fn on_cheat_detected(
    cheat_type: CheatType,
    hwid: &str,
    timestamp: u64,
    count: u32,
) {
    // 1. Get player_id (derived from Ed25519 public key)
    let player_id = derive_player_id(&CLIENT_KEYPAIR.public);
    
    // 2. Request action token from server
    let action_token = request_action_token(&player_id).await;
    
    // 3. Prepare payload with action token
    let payload = SignedPayload {
        action_data: CheatEvent {
            game_id: "my_game".to_string(),
            version: "1.0.0".to_string(),
            player_id: player_id.clone(),
            cheat_type: cheat_type as i32,
            hwid: hwid.to_string(),
            timestamp,
            detection_count: count,
        },
        action_token: action_token.clone(),
    };
    
    // 4. Sign payload with Ed25519 private key
    let signature = CLIENT_KEYPAIR.sign(&bincode::serialize(&payload).unwrap());
    
    // 5. Send signed payload to server
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        client.post("https://anticheat.example.com/cheat")
            .json(&SignedRequest {
                payload,
                signature: signature.to_bytes(),
            })
            .send()
            .await
            .ok(); // Best effort - don't block gameplay
    });
}

async fn request_action_token(player_id: &str) -> ActionToken {
    let client = reqwest::Client::new();
    let nonce = chrono::Utc::now().timestamp_millis() as u64;
    
    let response = client
        .post("https://anticheat.example.com/action-token")
        .json(&ActionTokenRequest {
            player_id: player_id.to_string(),
            nonce,
        })
        .send()
        .await
        .unwrap()
        .json::<ActionToken>()
        .await
        .unwrap();
    
    response
}
```

#### 2. Cloudflare Worker (Axum)

**Purpose:** Lightweight request routing, action token generation, and dual verification

**Endpoints:**

```rust
// POST /action-token - Request one-time action token
// Request:
{
    "player_id": "abc123def456...",  // BLAKE3 hash of Ed25519 public key
    "nonce": 1704067200123           // Client-generated timestamp (ms)
}

// Response:
{
    "player_id": "abc123def456...",
    "timestamp": 1704067200000,      // Server timestamp (seconds)
    "nonce": 1704067200123,          // Echoed back
    "token_hash": "8f4a3c...",       // BLAKE3(player_id || timestamp || nonce || server_secret)
    "expires_at": 1704067800000      // +5 minutes
}
```

```rust
// POST /cheat - Record a cheat event with dual verification
// Request:
{
    "payload": {
        "action_data": {
            "game_id": "my_game",
            "version": "1.0.0",
            "player_id": "abc123def456...",  // Derived from Ed25519 public key
            "cheat_type": 2,                 // CheatType::IntegrityViolation
            "hwid": "xyz789...",
            "timestamp": 1704067200000000000,  // Nanoseconds
            "detection_count": 1
        },
        "action_token": {
            "player_id": "abc123def456...",
            "timestamp": 1704067200000,
            "nonce": 1704067200123,
            "token_hash": "8f4a3c..."
        }
    },
    "signature": [64 bytes of Ed25519 signature]
}

// Response:
{
    "success": true,
    "recorded_at": 1704067200123456789,
    "player_state": {
        "player_id": "abc123def456...",
        "violation_count": 1,
        "last_violation": 1704067200000000000,
        "status": "flagged"
    }
}
```

```rust
// GET /status/{player_id}
// Query current player state using player_id (BLAKE3 hash of public key)

// Response:
{
    "player_id": "abc123def456...",
    "violation_count": 5,
    "last_violation": 1704067200000000000,
    "status": "banned",  // or "clean", "flagged", "watching"
    "recent_events": [
        {
            "cheat_type": 2,  // IntegrityViolation
            "timestamp": 1704067200000000000,
            "detection_count": 3
        }
    ]
}
```

```rust
// GET /query/{player_id_prefix}
// Query multiple players by player_id prefix

// Response:
{
    "players": [
        {
            "player_id": "abc123def456...",
            "status": "flagged",
            "violation_count": 3
        },
        {
            "player_id": "abc123xyz789...",
            "status": "clean",
            "violation_count": 0
        }
    ]
}
```

**Server-Side Verification Logic:**
```rust
async fn handle_cheat_event(req: SignedRequest) -> Result<CheatResponse> {
    // Route to appropriate Durable Object
    let do_stub = env.durable_object(PLAYER_DO).unwrap();
    let player_id = req.payload.action_data.player_id.clone();
    let player_do: PlayerObject = do_stub.get(&player_id).await?;
    
    // Delegate to DO for all verification and processing
    player_do.record_cheat(req).await
}

async fn handle_action_token_request(req: ActionTokenRequest) -> Result<ActionToken> {
    // Route to appropriate Durable Object
    let do_stub = env.durable_object(PLAYER_DO).unwrap();
    let player_do: PlayerObject = do_stub.get(&req.player_id).await?;
    
    // Generate action token
    player_do.generate_action_token(req.player_id, req.nonce).await
}

async fn handle_player_registration(req: RegistrationRequest) -> Result<RegistrationResponse> {
    // Route to appropriate Durable Object
    let do_stub = env.durable_object(PLAYER_DO).unwrap();
    let player_do: PlayerObject = do_stub.get(&req.player_id).await?;
    
    // Register player's public key
    player_do.register_player(req.player_id, req.public_key, req.hwid).await
}
```

**Data Structures:**

```rust
// Action token request (client → server)
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionTokenRequest {
    pub player_id: String,      // BLAKE3 hash of Ed25519 public key
    pub nonce: u64,            // Client-generated timestamp (ms)
}

// Action token (server → client)
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionToken {
    pub player_id: String,
    pub timestamp: u64,         // Server timestamp (seconds)
    pub nonce: u64,             // Echoed back
    pub token_hash: [u8; 32],  // BLAKE3(player_id || timestamp || nonce || server_secret)
    pub expires_at: u64,        // +5 minutes
}

// Signed payload structure
#[derive(Debug, Serialize, Deserialize)]
pub struct SignedPayload {
    pub action_data: CheatEvent,
    pub action_token: ActionToken,
}

// Signed request (client → server)
#[derive(Debug, Serialize, Deserialize)]
pub struct SignedRequest {
    pub payload: SignedPayload,
    pub signature: [u8; 64],   // Ed25519 signature
}

// Cheat event structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CheatEvent {
    pub game_id: String,
    pub version: String,
    pub player_id: String,     // BLAKE3 hash of Ed25519 public key
    pub cheat_type: i32,
    pub hwid: Option<String>,
    pub timestamp: u64,        // Nanoseconds since epoch
    pub detection_count: u32,
}

// Nonce tracker for replay prevention
pub struct NonceTracker {
    bloom_filter: BloomFilter<u64>,  // Probabilistic check
    recent_nonces: LruCache<u64, ()>,// Exact check for recent 10k
    window_start: u64,               // 5 minute sliding window
}

impl NonceTracker {
    pub fn new() -> Self {
        NonceTracker {
            bloom_filter: BloomFilter::new(100_000, 0.001), // 0.1% false positive rate
            recent_nonces: LruCache::new(10_000),
            window_start: chrono::Utc::now().timestamp(),
        }
    }
    
    pub fn is_used(&self, nonce: u64) -> bool {
        // First check LRU (exact)
        if self.recent_nonces.contains(&nonce) {
            return true;
        }
        // Then check Bloom filter (probabilistic)
        self.bloom_filter.contains(&nonce)
    }
    
    pub fn mark_used(&mut self, nonce: u64) -> Result<()> {
        // Add to both
        self.bloom_filter.insert(&nonce);
        self.recent_nonces.put(nonce, ());
        
        // Cleanup old nonces every 1000 calls
        if nonce % 1000 == 0 {
            self.cleanup_expired();
        }
        Ok(())
    }
    
    fn cleanup_expired(&mut self) {
        let now = chrono::Utc::now().timestamp();
        if now - self.window_start > 300 { // 5 minutes
            self.bloom_filter = BloomFilter::new(100_000, 0.001);
            self.recent_nonces = LruCache::new(10_000);
            self.window_start = now;
        }
    }
}

// Ed25519 key pair storage (client-side)
pub struct ClientKeypair {
    pub public_key: ed25519_dalek::PublicKey,
    pub private_key: ed25519_dalek::SecretKey,
}

impl ClientKeypair {
    pub fn new() -> Self {
        let mut csprng = OsRng {};
        let keypair = ed25519_dalek::Keypair::generate(&mut csprng);
        ClientKeypair {
            public_key: keypair.public,
            private_key: keypair.secret,
        }
    }
    
    pub fn derive_player_id(&self) -> String {
        format!("{:x}", blake3::hash(self.public_key.as_bytes()))
    }
}
```

// Useful for batch checks from game servers

// Response:
{
    "players": [
        {
            "player_uuid": "018f1234-5678-1234-5678-0123456789ab",
            "violation_count": 5,
            "status": "banned"
        },
        // ... more players
    ]
}
```

**Request Validation:**
- Validate UUID v7 format (must be time-ordered)
- Validate timestamp is within ±5 minutes of current time (prevent replay attacks)
- Validate game_id and version are whitelisted
- Rate limit per player_uuid (token bucket in DO)

#### 3. Durable Object (per Player ID)

**Purpose:** Manage player-specific cheat state with zero-latency SQLite

**SQLite Schema:**

```sql
-- In each Durable Object (per player_id)
CREATE TABLE cheat_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL,            -- BLAKE3 hash of Ed25519 public key
    cheat_type INTEGER NOT NULL,        -- CheatType enum
    timestamp INTEGER NOT NULL,         -- Nanoseconds
    detection_count INTEGER NOT NULL,
    hwid TEXT,
    created_at INTEGER NOT NULL         -- DO write timestamp
);

CREATE TABLE player_state (
    player_id TEXT PRIMARY KEY,         -- BLAKE3 hash of Ed25519 public key
    public_key TEXT,                    -- Ed25519 public key (hex)
    hwid TEXT,                          -- Hardware ID (optional)
    violation_count INTEGER NOT NULL DEFAULT 0,
    last_violation INTEGER,
    last_status TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE used_nonces (
    nonce INTEGER PRIMARY KEY,
    player_id TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

-- Indexes for fast queries
CREATE INDEX idx_events_by_time ON cheat_events(timestamp DESC);
CREATE INDEX idx_state_by_status ON player_state(last_status);
CREATE INDEX idx_events_by_player ON cheat_events(player_id, timestamp DESC);
CREATE INDEX idx_used_nonces_expires ON used_nonces(expires_at);
CREATE TABLE used_nonces (
    nonce INTEGER PRIMARY KEY,
    player_id TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_cheat_events_timestamp ON cheat_events(timestamp);
CREATE INDEX idx_used_nonces_expires ON used_nonces(expires_at);
```

-- Indexes for fast queries
CREATE INDEX idx_events_by_time ON cheat_events(timestamp DESC);
CREATE INDEX idx_events_by_type ON cheat_events(cheat_type, timestamp DESC);
```

**Durable Object Logic:**

```rust
impl DurableObjectState {
    // Record a cheat event (synchronous, no await!)
    pub fn record_cheat(&mut self, event: CheatEvent) -> PlayerState {
        // Insert into SQLite (zero latency)
        self.db.execute(
            "INSERT INTO cheat_events (cheat_type, timestamp, detection_count, hwid, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.cheat_type, event.timestamp, event.detection_count, event.hwid, now_ns()]
        );

        // Update player state
        let count = self.violation_count + 1;
        let status = self.calculate_status(count);
        
        self.db.execute(
            "INSERT OR REPLACE INTO player_state (player_uuid, violation_count, last_violation, last_status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![self.player_uuid, count, event.timestamp, status, now_ns()]
        );

        // Write to KV with proper key naming
        let key = format!("{}|{}|{}", event.game_id, event.version, self.player_uuid);
        let value = CheatEventRecord {
            player_uuid: self.player_uuid.clone(),
            violation_count: count,
            last_violation: event.timestamp,
            status: status.clone(),
            recent_events: self.get_recent_events(10), // Last 10 events
        };
        
        self.kv.put(&key, &value).unwrap(); // Fire and forget

        PlayerState {
            player_uuid: self.player_uuid.clone(),
            violation_count: count,
            last_violation: event.timestamp,
            status,
        }
    }

    // Get current player state
    pub fn get_state(&self) -> PlayerState {
        self.db.query_row(
            "SELECT violation_count, last_violation, last_status FROM player_state WHERE player_uuid = ?1",
            params![self.player_uuid],
            |row| PlayerState::from_row(row)
        ).unwrap_or_default()
    }

    // Calculate status based on violations
    fn calculate_status(&self, count: u32) -> String {
        match count {
            0 => "clean".to_string(),
            1..=2 => "flagged".to_string(),
            3..=5 => "watching".to_string(),
            _ => "banned".to_string(),
        }
    }
}
```

**Why per-player DO?**
- Each player gets isolated state (no contention)
- SQLite queries are synchronous (zero latency)
- Easy to scale - just create more DOs
- Horizontal scaling by player UUID hash

#### 4. KV Store

**Purpose:** Persistent, queryable storage for cheat events

**Key Format:** `${game_id}|${version}|${player_uuid}`

**Benefits of this format:**
- **Prefix filtering:** Query all players for a game/version: `kv.list({ prefix: "my_game|1.0.0|" })`
- **Game isolation:** Different games don't interfere
- **Version tracking:** Support multiple game versions
- **Player isolation:** Each player has their own record

**Value Format:**
```json
{
    "player_uuid": "018f1234-5678-1234-5678-0123456789ab",
    "violation_count": 5,
    "last_violation": 1704067200000000000,
    "status": "banned",
    "recent_events": [
        {
            "cheat_type": 2,
            "timestamp": 1704067200000000000,
            "detection_count": 3
        },
        {
            "cheat_type": 0,
            "timestamp": 1704067100000000000,
            "detection_count": 1
        }
    ],
    "first_violation": 1704067000000000000,
    "updated_at": 1704067200123456789
}
```

**Query Examples:**

```rust
// Get specific player
let key = "my_game|1.0.0|018f1234-5678-1234-5678-0123456789ab";
let value = kv.get(&key).await?;

// Get all players for a game/version
let list = kv.list()
    .prefix("my_game|1.0.0|")
    .limit(100)
    .execute()
    .await?;

// Get players with recent violations
let now = Utc::now().timestamp_nanos_opt().unwrap();
let one_hour_ago = now - 3_600_000_000_000; // 1 hour in nanoseconds
let list = kv.list()
    .prefix("my_game|1.0.0|")
    .execute()
    .await?
    .into_iter()
    .filter(|v| v.last_violation > one_hour_ago)
    .collect::<Vec<_>>();
```

#### 5. Docker Containers (Optional)

**Purpose:** Heavy processing that can't run in WASM

**Use Cases:**
- Complex ML-based cheat detection
- Behavioral analysis requiring heavy computation
- Historical data analysis
- Batch processing of player logs

**When to Use:**
- Default: Use SQLite in Durable Object (fast, simple)
- Only for: CPU-intensive tasks that can't be simplified

**Integration:**
```rust
// In Durable Object
pub fn process_heavy_analysis(&self, player_uuid: &str) -> AnalysisResult {
    // Check if heavy analysis is needed
    if self.needs_heavy_analysis() {
        // Call Docker container via fetch
        let result = fetch(&format!(
            "https://docker-container.example.com/analyze/{}",
            player_uuid
        )).await?;
        
        // Store result in KV
        self.kv.put(
            &format!("analysis|{}", player_uuid),
            &result
        ).unwrap();
        
        result
    } else {
        // Use simple SQLite-based analysis
        self.simple_analysis()
    }
}
```

## Cheat Type Enum

```rust
pub enum CheatType {
    MemoryTampering = 0,      // Protected<T> trap triggered
    ValueFreeze = 1,          // Value freeze detected
    IntegrityViolation = 2,   // Code/memory integrity compromised
    SpeedHack = 3,            // Timestamp manipulation
    MacroDetection = 4,      // Hardware macro detected
    ProcessInjection = 5,     // Malicious process found
    DebuggerAttached = 6,     // Debugger detected
    VmDetected = 7,           // Running in VM
    Unknown = 99,
}

impl CheatType {
    pub fn severity(&self) -> u32 {
        match self {
            CheatType::IntegrityViolation => 10,
            CheatType::MemoryTampering => 5,
            CheatType::ValueFreeze => 3,
            CheatType::SpeedHack => 7,
            CheatType::ProcessInjection => 10,
            CheatType::DebuggerAttached => 4,
            CheatType::VmDetected => 2,
            CheatType::MacroDetection => 1,
            CheatType::Unknown => 1,
        }
    }
}
```

## Integration with Other Systems

This sidecar service is designed to be **consumed by other systems**, not to make enforcement decisions itself.

### Example: Game Server Integration

```rust
// In game server (Rust)
async fn check_player_auth(player_uuid: &str) -> Result<bool> {
    // 1. Check existing auth server
    let auth_result = auth_client.verify(player_uuid).await?;
    
    if !auth_result.is_valid {
        return Ok(false);
    }

    // 2. Check anti-cheat status
    let anticheat_response = reqwest::get(format!(
        "https://anticheat.example.com/status/{}/{}",
        "my_game", player_uuid
    )).await?.json::<PlayerState>().await?;

    // 3. Decide based on YOUR rules (not anti-cheat system)
    match anticheat_response.status.as_str() {
        "clean" => Ok(true),
        "flagged" => Ok(true),  // Allow but monitor
        "watching" => Ok(true), // Allow but log heavily
        "banned" => Ok(false),
        _ => Ok(true),
    }
}
```

### Example: Ban System Integration

```rust
// In ban management service (separate system)
async fn sync_bans() {
    // Query all banned players from anti-cheat KV
    let list = kv.list()
        .prefix("my_game|1.0.0|")
        .execute()
        .await?;

    for item in list {
        let state: CheatEventRecord = serde_json::from_str(&item.value)?;
        
        if state.status == "banned" {
            // Enforce ban in YOUR system
            ban_service.ban_player(
                &state.player_uuid,
                "Anti-cheat violation",
                state.last_violation
            ).await?;
        }
    }
}
```

### Example: Analytics Integration

```rust
// In analytics system (separate)
async fn collect_cheat_metrics() {
    // Query all players with violations in last hour
    let now = Utc::now().timestamp_nanos_opt().unwrap();
    let one_hour_ago = now - 3_600_000_000_000;
    
    let list = kv.list()
        .prefix("my_game|1.0.0|")
        .execute()
        .await?
        .into_iter()
        .filter(|v| v.last_violation > one_hour_ago);

    // Aggregate metrics
    let mut by_type = HashMap::new();
    
    for item in list {
        let state: CheatEventRecord = serde_json::from_str(&item.value)?;
        
        for event in &state.recent_events {
            *by_type.entry(event.cheat_type).or_insert(0) += 1;
        }
    }
    
    // Send to your analytics system
    analytics_client.report_cheat_types(by_type).await?;
}
```

## Performance Characteristics

### Latency Targets

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| Client → CF Worker | < 5ms | Network only |
| CF Worker → DO | < 1ms | Same data center |
| DO SQLite insert | < 1ms | Synchronous, no await |
| DO → KV write | < 2ms | Fire and forget |
| **Total** | **< 10ms** | End-to-end |

### Throughput Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Cheat events/sec | 50,000 | With proper DO sharding |
| Status queries/sec | 100,000 | KV is highly scalable |
| Concurrent players | 1,000,000 | 1 DO per player UUID |

### Scaling Strategy

**Horizontal Scaling:**
- Each player UUID maps to a specific Durable Object
- Hash partitioning: `DO_ID = hash(player_uuid) % NUM_DOs`
- Scale by increasing NUM_DOs (create more DO instances)
- Each DO is isolated, no contention

**Vertical Scaling:**
- Each DO can handle ~10,000 players
- SQLite operations are synchronous and fast
- No async overhead for common operations

## Security Considerations

### Dual Verification System

The system implements a **dual verification** mechanism for maximum security:

1. **Client Identity Verification (Ed25519)**
   - Client generates Ed25519 key pair at first launch
   - Public key registered with server linked to HardwareID
   - All high-value actions signed with private key
   - Server verifies signature against registered public key

2. **Action Authorization (BLAKE3 Action Tokens)**
   - Server generates one-time action tokens using BLAKE3
   - Token = BLAKE3(player_id || timestamp || nonce || server_secret)
   - Tokens are time-limited (±5 minutes) and single-use
   - Prevents replay attacks and payload tampering

### Complete Authentication Flow

```
1. Client generates Ed25519 key pair (first launch)
   ↓
2. Client registers public key with server
   ↓
3. Client requests action token from server
   ↓
4. Server generates action_token = BLAKE3(player_id || timestamp || nonce || server_secret)
   ↓
5. Server stores nonce (to prevent reuse) in sliding window + Bloom filter
   ↓
6. Server sends action_token to client
   ↓
7. Client signs payload (including action_token) with Ed25519 private key
   ↓
8. Server verifies:
   - ✅ Ed25519 signature valid (using registered public key)
   - ✅ action_token valid (BLAKE3 hash matches)
   - ✅ action_token not used before (one-time via nonce check)
   - ✅ timestamp recent (time-limited ±5 minutes)
```

### Request Security

1. **Timestamp Validation**
   - Must be within ±5 minutes of server time
   - Prevents replay attacks
   - Uses nanosecond precision

2. **Action Token Validation**
   - BLAKE3 hash verification (server secret)
   - Nonce uniqueness check (sliding window + Bloom filter)
   - Time-bound expiry (automatic)

3. **Rate Limiting**
   - Per player_uuid: 100 events/minute
   - Per IP: 1,000 events/minute
   - Implemented in Durable Object (token bucket)

4. **Input Validation**
   - UUID v7 format validation (time-ordered)
   - Game ID whitelist
   - Version format validation (semver)

### Data Security

1. **Encryption in Transit**
   - TLS 1.3 only
   - HSTS enabled
   - Certificate transparency

2. **Data Retention**
   - KV: 90 days default
   - SQLite in DO: 30 days (DO limits)
   - Configurable per game

3. **Privacy**
   - HWID is optional
   - Minimal data collection
   - GDPR compliant (by design)

### Access Control

1. **API Authentication**
   - Simple API key (for game servers)
   - Per-game keys
   - Key rotation support

2. **CORS**
   - Strict origin whitelist
   - Only allowed game domains

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_record_cheat_event() {
        let mut state = DurableObjectState::new("test_uuid");
        let event = CheatEvent {
            game_id: "test_game".to_string(),
            version: "1.0.0".to_string(),
            player_uuid: "test_uuid".to_string(),
            cheat_type: CheatType::IntegrityViolation as i32,
            hwid: "test_hwid".to_string(),
            timestamp: Utc::now().timestamp_nanos_opt().unwrap(),
            detection_count: 1,
        };
        
        let player_state = state.record_cheat(event);
        
        assert_eq!(player_state.violation_count, 1);
        assert_eq!(player_state.status, "flagged");
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_cheat_detection() {
    // 1. Start test server
    let server = start_test_server().await;
    
    // 2. Simulate cheat event from client
    let response = reqwest::Client::new()
        .post(&format!("{}/cheat", server.url()))
        .json(&test_cheat_event())
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    // 3. Query player state
    let status_response = reqwest::get(format!(
        "{}/status/test_game/1.0.0/{}",
        server.url(),
        TEST_PLAYER_UUID
    ))
    .await
    .unwrap()
    .json::<PlayerState>()
    .await
    .unwrap();
    
    assert_eq!(status_response.violation_count, 1);
    assert_eq!(status_response.status, "flagged");
}
```

### Load Tests

```bash
# Use wrk or similar for load testing
wrk -t12 -c400 -d30s \
    -s cheat_event.lua \
    https://anticheat.example.com/cheat
```

## Deployment

### Cloudflare Worker Deployment

```bash
# Install wrangler
npm install -g wrangler

# Login
wrangler login

# Deploy worker
wrangler deploy --name maxion-anticheat

# Create Durable Object namespace
wrangler d1 create maxion-anticheat-db
wrangler d1 execute maxion-anticheat-db --file=./schema.sql

# Create KV namespace
wrangler kv:namespace create "CHEAT_EVENTS"
```

### Configuration

```toml
# wrangler.toml
name = "maxion-anticheat"
main = "src/worker.rs"
compatibility_date = "2024-01-01"

[env.production]
workers_dev = false
routes = [
  { pattern = "anticheat.example.com/*", zone_name = "example.com" }
]

[[env.production.durable_objects.bindings]]
name = "PLAYER_STATE"
class_name = "PlayerStateDO"

[[env.production.kv_namespaces]]
binding = "CHEAT_KV"
id = "your_kv_namespace_id"
```

### Monitoring

```rust
// Add metrics to worker
use worker::Metrics;

let metrics = Metrics::new("maxion_anticheat");

// Track events
metrics.counter!("cheat_events_total", 1);
metrics.histogram!("cheat_event_latency_ms", latency_ms);
metrics.gauge!("active_players", active_players);
```

## Cost Analysis

### Cloudflare Pricing (Estimates)

| Resource | Free Tier | Paid Tier | Notes |
|----------|-----------|-----------|-------|
| Workers | 100k req/day | $0.50/M req | First 100k free |
| DO CPU | 400ms/req | $0.15/M ms | SQLite is fast |
| DO Storage | 128MB/DO | $0.50/GB | 1GB limit per DO |
| KV Reads | 100k/day | $0.50/M read | High read limit |
| KV Writes | 1k/day | $5.00/M write | Higher write cost |
| KV Storage | 1GB | $0.50/GB | Cheaper than DO |

**Estimated Monthly Cost (1M active players):**
- Workers: $0.50 (under free tier)
- DO CPU: $1,500 (1B events × 1ms × $0.15/M)
- DO Storage: $50 (100 GB across DOs)
- KV Reads: $50 (100M reads)
- KV Writes: $250 (50M writes)
- KV Storage: $10 (20 GB)
- **Total: ~$1,910/month**

**Cost Optimization:**
- Batch cheat events (reduce KV writes)
- Use SQLite in DO for recent events, KV for long-term
- Implement TTL for old data
- Use DO list operations instead of KV scans

## Project Structure

```
crates/maxion-anticheat/
├── Cargo.toml
├── src/
│   ├── main.rs           # Worker entry point
│   ├── worker.rs         # Axum router and handlers
│   ├── do/
│   │   ├── mod.rs        # DO modules
│   │   ├── player.rs     # PlayerStateDO
│   │   └── mod.rs        # DO state and logic
│   ├── types.rs          # Shared types (CheatEvent, PlayerState, etc.)
│   ├── kv.rs             # KV operations
│   └── utils.rs          # Utilities (time, validation)
├── tests/
│   ├── integration.rs    # Integration tests
│   └── load_test.rs      # Load tests
└── wrangler.toml         # CF Worker config

# Client integration (in maxion-core)
crates/maxion-core/
├── src/
│   └── ffi/
│       └── anticheat.rs  # FFI for Unity callback
└── examples/
    └── cheat_callback_demo.rs
```

## Implementation Timeline

**See [TASKS.md](./TASKS.md) for detailed implementation checklist**

### Phase 1: Core Infrastructure (7 days)
- [ ] Setup Cloudflare Worker project
- [ ] Implement Axum router
- [ ] Create Durable Object schema
- [ ] Implement `/cheat` endpoint
- [ ] Implement `/status` endpoint
- [ ] Add request validation
- [ ] Deploy to CF Workers

### Phase 2: Client Integration (5 days)
- [ ] Update Unity callback to call `/cheat`
- [ ] Add retry logic (best effort)
- [ ] Add local buffering (offline support)
- [ ] Test with cheat_callback_demo
- [ ] Update docs

### Phase 3: Query Optimization (5 days)
- [ ] Implement `/query` endpoint (batch)
- [ ] Optimize KV key naming
- [ ] Add prefix filtering
- [ ] Add caching layer (optional)
- [ ] Performance testing

### Phase 4: Docker Integration (3 days) - OPTIONAL
- [ ] Setup Docker container
- [ ] Implement heavy analysis endpoint
- [ ] Integrate with DO
- [ ] Test ML pipeline

### Phase 5: Testing & Polish (5 days)
- [ ] Unit tests (target 90% coverage)
- [ ] Integration tests
- [ ] Load tests (target 50k req/s)
- [ ] Security audit
- [ ] Documentation

**Total: 25 days (5 weeks)** - Can be done in parallel phases

## Success Criteria

**See [TASKS.md](./TASKS.md) for detailed acceptance criteria and testing checklist**

### Functional
- ✅ Cheat events recorded within 10ms
- ✅ Player state queryable via KV
- ✅ Supports prefix filtering
- ✅ Nanosecond timestamp accuracy
- ✅ Uuid v7 support

### Performance
- ✅ < 10ms end-to-end latency
- ✅ 50,000 cheat events/sec throughput
- ✅ 100,000 status queries/sec
- ✅ < 1GB memory per DO

### Reliability
- ✅ 99.9% uptime (Cloudflare SLA)
- ✅ Zero data loss (DO replication)
- ✅ Graceful degradation (offline mode)

### Security
- ✅ TLS 1.3 only
- ✅ Replay attack prevention
- ✅ Rate limiting
- ✅ Input validation

## Future Enhancements

### Short Term (1-2 months)
- [ ] Add webhook notifications (to game servers)
- [ ] Add batch import from existing systems
- [ ] Add admin dashboard
- [ ] Add export to CSV/JSON

### Medium Term (3-6 months)
- [ ] Add machine learning-based detection
- [ ] Add behavioral analysis
- [ ] Add anomaly detection
- [ ] Add real-time alerts

### Long Term (6+ months)
- [ ] Add pattern management system
- [ ] Add automated ban escalation
- [ ] Add player reputation scoring
- [ ] Add fraud detection

## Related Documentation

- **Architecture:** `ARCHITECTURE_SUMMARY.md` - Detailed architecture diagrams and component breakdown
- **Tasks:** `TASKS.md` - Implementation checklist and acceptance criteria
- **Trap System:** `docs/06_security/006_trap.md` - Client-side protection
- **Demo:** `crates/maxion-core/examples/cheat_callback_demo.rs` - Example callback
- **Main Plan:** `../007_xigncode3.md` - Full anti-cheat feature plan
- **Project:** `../README.md` - Project overview

## Questions?

Refer to:
- `../ISSUES.md` for issue tracking
- Project maintainers for clarification

---

**Last Updated:** 2025-01-21  
**Version:** 2.0 (Lightweight Sidecar)  
**Status:** 📝 Planning  
**Maintainer:** Maxion Protector Team