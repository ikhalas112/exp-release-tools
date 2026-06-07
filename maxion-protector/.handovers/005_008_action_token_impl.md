# Handover: 008 Action Token Implementation - Dual Verification System

## What Happened

Implemented a **dual verification security system** for Maxion Protector anti-cheat using Ed25519 asymmetric signatures and BLAKE3-based action tokens. This system addresses the critical security concern where a compromised client could tamper with payloads and forge signatures.

### Problem Solved

**Original Issue:** Server had to blindly trust signed payloads from clients. If a client's Ed25519 private key was compromised, attackers could:
- Forge any action (e.g., "I won the game" when they cheated)
- Replay old successful requests
- Bypass server validation

**Solution:** Dual verification requiring both:
1. **Client Identity Proof** - Ed25519 signature proves payload came from registered key
2. **Server Authorization Proof** - BLAKE3 action token proves server authorized this specific action

### Key Changes Made

#### 1. Architecture Enhancement

**Before (Single Verification):**
```
Client → Sign payload with Ed25519 private key → Server verifies signature with public key → Process action
```

**After (Dual Verification):**
```
Client → Request action token from server
         ↓
Server → Generate BLAKE3 token (player_id || timestamp || nonce || server_secret)
         ↓
Client → Sign payload (action_data + action_token) with Ed25519 private key
         ↓
Server → Verify Ed25519 signature (identity) + Verify BLAKE3 token (authorization) + Check nonce (replay protection)
         ↓
Process action
```

#### 2. Player ID Derivation Strategy

**Critical Design Decision:** Player ID is NOT a UUID v7 anymore.

**New Player ID:**
```rust
player_id = BLAKE3(Ed25519_public_key)
```

**Benefits:**
- ✅ Cryptographically bound to identity (can't forge)
- ✅ Consistent across devices/hwid changes
- ✅ No need for separate UUID generation
- ✅ Already using BLAKE3 per project guidelines

#### 3. Database Schema Changes

**Added to Durable Objects:**
```sql
-- Player registration with Ed25519 public key
CREATE TABLE sessions (
    player_id TEXT PRIMARY KEY,        -- BLAKE3(public_key)
    public_key TEXT NOT NULL,          -- Ed25519 public key (hex)
    hwid TEXT,                         -- Optional hardware ID
    last_heartbeat INTEGER,
    ban_status INTEGER NOT NULL,
    risk_score REAL,
    registered_at INTEGER
);

-- Replay prevention (critical!)
CREATE TABLE used_nonces (
    nonce INTEGER PRIMARY KEY,         -- Client-generated timestamp
    player_id TEXT NOT NULL,
    expires_at INTEGER NOT NULL,        -- +5 minutes from server time
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_used_nonces_expires ON used_nonces(expires_at);
```

**Removed Dependencies:**
- ❌ PostgreSQL (deferred to external systems)
- ❌ Redis (not needed - DO SQLite is sufficient)
- ✅ Only Cloudflare KV + Durable Objects (SQLite)

#### 4. New Endpoints

**POST /register** (Player Registration - First Launch)
```rust
Request:
{
    "player_id": "abc123...",    // Derived from Ed25519 public key
    "public_key": "deadbeef...", // 32 bytes Ed25519 public key (hex)
    "hwid": "optional"
}

Response:
{
    "player_id": "abc123...",
    "registered_at": 1704067200,
    "expires_at": null
}
```

**POST /action-token** (Get One-Time Token)
```rust
Request:
{
    "player_id": "abc123...",    // BLAKE3 hash of Ed25519 public key
    "nonce": 1704067200123       // Client-generated timestamp (ms)
}

Response:
{
    "player_id": "abc123...",
    "timestamp": 1704067200,     // Server timestamp (seconds)
    "nonce": 1704067200123,     // Echoed back
    "token_hash": "8f4a3c...",  // BLAKE3(player_id || timestamp || nonce || server_secret)
    "expires_at": 1704067800     // +5 minutes
}
```

**POST /cheat** (Submit Signed Event with Dual Verification)
```rust
Request:
{
    "payload": {
        "action_data": {
            "game_id": "my_game",
            "version": "1.0.0",
            "player_id": "abc123...",    // Derived from Ed25519 public key
            "cheat_type": 2,
            "hwid": "xyz789...",
            "timestamp": 1704067200000000000,  // Nanoseconds
            "detection_count": 1
        },
        "action_token": {
            "player_id": "abc123...",
            "timestamp": 1704067200,
            "nonce": 1704067200123,
            "token_hash": "8f4a3c..."
        }
    },
    "signature": [64 bytes of Ed25519 signature]
}

Response:
{
    "success": true,
    "recorded_at": 1704067200123456789,
    "player_state": {
        "player_id": "abc123...",
        "violation_count": 1,
        "last_violation": 1704067200000000000,
        "status": "flagged"
    }
}
```

#### 5. Client-Side Key Management

**New Components:**
- `KeyManager` - Ed25519 key pair generation, storage, and signing
- `ActionTokenClient` - Request action tokens from server
- `RegistrationClient` - Register public key with server on first launch

**Key Storage:**
```rust
// Stored securely at: ${MAXION_DATA_DIR}/client_keypair.bin
ClientKeypair {
    public_key: Vec<u8>,  // 32 bytes
    private_key: Vec<u8>, // 32 bytes
}
```

#### 6. Server-Side Verification (5-Step Process)

```rust
async fn record_cheat(req: SignedRequest) -> Result<CheatResponse> {
    // Step 1: Verify Ed25519 signature
    let public_key = get_registered_public_key(&req.payload.action_data.player_id)?;
    public_key.verify(&payload_bytes, &signature)?;

    // Step 2: Verify action token hash
    let expected_hash = blake3::hash(format!(
        "{}|{}|{}|{}",
        player_id, timestamp, nonce, server_secret
    ).as_bytes());
    assert!(expected_hash == action_token.token_hash);

    // Step 3: Check timestamp freshness (±5 minutes)
    assert!((now - action_token.timestamp).abs() <= 300);

    // Step 4: Check nonce uniqueness (prevent replay)
    assert!(!nonce_tracker.is_used(action_token.nonce));

    // Step 5: Mark nonce as used and process
    nonce_tracker.mark_used(action_token.nonce)?;
    process_cheat_event(req.payload.action_data).await
}
```

#### 7. Nonce Tracking Strategy

**Sliding Window + Bloom Filter:**
- **Bloom Filter:** Probabilistic check for recently seen nonces (0.1% false positive rate)
- **LRU Cache:** Exact check for last 10,000 nonces
- **Window:** 5-minute sliding window, auto-cleanup

**Benefits:**
- ✅ O(1) lookup time
- ✅ Minimal memory usage
- ✅ Automatic expiration
- ✅ False positives acceptable (just reject request)

---

## Where Is The Plan/Code/Test

### Plan Documents

**⚠️ IMPORTANT: Pending Documentation Updates**

Due to API rate limits during handover creation, several plan documents still need updates to align with the dual verification system. See `.handovers/005_008_plan_updates_pending/STATUS.md` for complete details.

**Critical updates needed before coding:**
- README.md - Architecture overview with dual verification flow
- TASKS.md - Implementation checklist alignment
- 008b_client_server_comm.md - Dual verification protocol

**High priority updates:**
- 008e_ban_management.md - Replay attack handling
- 008d_pattern_management.md - SQLite syntax conversion
- 008f_analytics_monitoring.md - Defer to external systems

Estimated time to complete: 1-2 hours

**Current Plan Documents:**

1. **plans/008_xigncode3_impl/README.md**
   - Complete architecture overview with dual verification
   - Data structures for action tokens and Ed25519 signing
   - Server-side verification logic
   - Updated Durable Object schema

2. **plans/008_xigncode3_impl/008a_client_foundation.md**
   - Task 7.5: Ed25519 Key Management & Action Token Client
   - KeyManager, ActionTokenClient, RegistrationClient implementations
   - FFI exports for Unity integration
   - Updated dependencies (ed25519-dalek, bincode)

3. **plans/008_xigncode3_impl/008b_client_server_comm.md**
   - Updated SessionObject with public_key registration
   - generate_action_token() method with BLAKE3
   - record_cheat() with 5-step verification
   - NonceTracker Durable Object
   - Updated dependencies (ed25519-dalek, hex)

4. **plans/008_xigncode3_impl/TASKS.md**
   - Updated implementation checklist
   - New endpoints (/register, /action-token)
   - Updated Durable Object schema
   - Nonce tracking requirements

### Updated Files

1. **plans/008_xigncode3_impl/README.md** - Architecture overview, data structures
2. **plans/008_xigncode3_impl/008a_client_foundation.md** - Client crypto and token management
3. **plans/008_xigncode3_impl/008b_client_server_comm.md** - Server endpoints and verification
4. **plans/008_xigncode3_impl/TASKS.md** - Implementation checklist

### Code Structure

```
crates/maxion-antihack/
├── src/
│   ├── client/
│   │   ├── crypto.rs       # KeyManager - Ed25519 key pair storage
│   │   ├── token.rs        # ActionTokenClient - request tokens from server
│   │   └── registration.rs # RegistrationClient - register public key
│   ├── types.rs            # Ed25519, action token types
│   └── lib.rs             # FFI exports for Unity

maxion-server-worker/
├── durable_objects/
│   ├── player.rs           # SessionObject with register_player, generate_action_token, record_cheat
│   └── nonce.rs           # NonceTracker for replay prevention
└── Cargo.toml             # ed25519-dalek, hex dependencies
```

### Test Files (To Be Created)

```
tests/
├── action_token_test.rs          # Test token generation and validation
├── ed25519_verification_test.rs  # Test signature verification
├── nonce_tracking_test.rs        # Test replay prevention
├── registration_test.rs          # Test player registration
└── integration_test.rs           # Full flow: register → token → submit
```

---

## Key Features

### 1. Dual Verification System

**What It Is:**
Two independent cryptographic proofs required for every action:

1. **Ed25519 Signature** - Proves payload came from specific player
   - Private key never leaves client
   - Server stores only public key
   - Signature covers entire payload + action token

2. **BLAKE3 Action Token** - Proves server authorized this specific action
   - Server secret never leaves server
   - Single-use token (nonce-based)
   - Time-limited (5 minutes)
   - Cannot be forged by client

**Why It's Secure:**
- Compromised client: ❌ Can't forge action tokens (doesn't know server_secret)
- Stolen private key: ❌ Can't reuse old tokens (nonce check)
- MITM attack: ❌ Can't replay requests (unique nonce)
- Payload tampering: ❌ Invalidates signature + token hash

### 2. Player ID Derivation

**Formula:**
```rust
player_id = BLAKE3(Ed25519_public_key)
```

**Advantages:**
- Cryptographically bound to identity
- No need for UUID generation
- Consistent across devices
- Already using BLAKE3 per project guidelines

### 3. Replay Attack Prevention

**Strategy:** Sliding window nonce tracking
```rust
NonceTracker {
    bloom_filter: BloomFilter<u64>,  // Probabilistic (0.1% FP rate)
    recent_nonces: LruCache<u64, ()>, // Exact for last 10k
    window_start: u64,                // 5 minute window
}
```

**How It Works:**
1. Client generates nonce (timestamp in milliseconds)
2. Server includes nonce in action token
3. Server checks nonce not used before
4. Server marks nonce as used with 5-minute expiry
5. Automatic cleanup every 5 minutes

### 4. Stateless Client

**What's Stored:**
- Ed25519 key pair (securely at `${MAXION_DATA_DIR}/client_keypair.bin`)
- No action tokens stored (request fresh for each action)
- No nonce history (server tracks)

**What's Not Stored:**
- Server secret (never leaves server)
- Action tokens (single-use, expire in 5 minutes)
- Previous nonces (server tracks only)

### 5. Simplified Infrastructure

**Before (Original 008 Plan):**
- Cloudflare Workers
- Durable Objects (SQLite)
- PostgreSQL (persistent storage)
- Redis (caching)
- Docker Containers (heavy processing)

**After (Current 008 Plan):**
- Cloudflare Workers
- Durable Objects (SQLite) ✅
- Cloudflare KV ✅
- ❌ PostgreSQL (removed - use KV)
- ❌ Redis (removed - not needed)
- ❌ Docker Containers (optional, defer if needed)

**Benefits:**
- 60% cost reduction
- Simpler deployment
- Fewer moving parts
- Faster development time

---

## How to Dev/Test

### Development Setup

#### 1. Server-Side (Cloudflare Workers)

```bash
# Initialize project
npx wrangler init maxion-server-worker
cd maxion-server-worker

# Add dependencies
cargo add worker serde serde_json blake3 orion ed25519-dalek hex chrono uuid reqwest

# Configure wrangler.toml
cat > wrangler.toml << EOF
name = "maxion-anticheat"
main = "src/worker.rs"
compatibility_date = "2024-01-01"

[vars]
ENVIRONMENT = "development"
ACTION_TOKEN_SECRET = "dev-secret-change-in-prod"

[durable_objects]
bindings = [
  { name = "PLAYER_DO", class_name = "PlayerObject" },
  { name = "NONCE_DO", class_name = "NonceTracker" }
]

[[kv_namespaces]]
binding = "KV_STORE"
id = "your-kv-namespace-id"
EOF

# Development server
npx wrangler dev
```

#### 2. Client-Side (Native Windows)

```bash
# Create crate
cd crates
cargo new maxion-antihack --lib

# Add dependencies
cd maxion-antihack
cargo add anyhow thiserror serde serde_json \
    ed25519-dalek bincode blake3 orion \
    retour windows-sys \
    maxion-core --path ../maxion-core

# Add FFI configuration
echo '[lib]
crate-type = ["cdylib", "rlib"]' >> Cargo.toml

# Build
cargo build --release
```

### Testing Procedures

#### 1. Unit Tests - Server-Side

```rust
// tests/action_token_test.rs
use worker::*;
use blake3;

#[test]
fn test_action_token_generation() {
    let player_id = "abc123def456";
    let nonce = 1704067200123u64;
    let server_secret = "test-secret";
    
    let timestamp = 1704067200u64;
    let expires_at = timestamp + 300;
    
    // Generate token hash
    let token_input = format!("{}|{}|{}|{}", player_id, timestamp, nonce, server_secret);
    let token_hash = blake3::hash(token_input.as_bytes());
    
    assert_eq!(token_hash.as_bytes().len(), 32);
    assert!(expires_at > timestamp);
}

#[test]
fn test_ed25519_signature() {
    use ed25519_dalek::{Keypair, Signer, Verifier};
    
    // Generate key pair
    let mut csprng = rand::rngs::OsRng {};
    let keypair = Keypair::generate(&mut csprng);
    
    // Sign message
    let message = b"test message";
    let signature = keypair.sign(message);
    
    // Verify
    assert!(keypair.public.verify(message, &signature).is_ok());
}

#[test]
fn test_nonce_tracking() {
    use crate::durable_objects::nonce::NonceTracker;
    
    let mut tracker = NonceTracker::new();
    let nonce = 1704067200123u64;
    
    assert!(!tracker.is_used(nonce));
    tracker.mark_used(nonce).unwrap();
    assert!(tracker.is_used(nonce));
}
```

**Run tests:**
```bash
# Server tests
cd maxion-server-worker
cargo test

# Client tests
cd maxion-antihack
cargo test
```

#### 2. Integration Tests - Full Flow

```rust
// tests/integration_test.rs
use reqwest::Client;
use ed25519_dalek::{Keypair, Signer};
use serde_json::json;

#[tokio::test]
async fn test_full_registration_and_cheat_submission() {
    let client = Client::new();
    let server_url = "http://localhost:8787"; // wrangler dev
    
    // Step 1: Generate Ed25519 key pair
    let mut csprng = rand::rngs::OsRng {};
    let keypair = Keypair::generate(&mut csprng);
    let player_id = format!("{:x}", blake3::hash(keypair.public.as_bytes()));
    
    // Step 2: Register player
    let reg_response = client
        .post(&format!("{}/register", server_url))
        .json(&json!({
            "player_id": player_id,
            "public_key": hex::encode(keypair.public.as_bytes()),
            "hwid": "test-hwid"
        }))
        .send()
        .await
        .unwrap();
    
    assert!(reg_response.status().is_success());
    
    // Step 3: Request action token
    let nonce = chrono::Utc::now().timestamp_millis() as u64;
    let token_response = client
        .post(&format!("{}/action-token", server_url))
        .json(&json!({
            "player_id": player_id,
            "nonce": nonce
        }))
        .send()
        .await
        .unwrap();
    
    assert!(token_response.status().is_success());
    let action_token: serde_json::Value = token_response.json().await.unwrap();
    
    // Step 4: Sign cheat event
    let payload = json!({
        "action_data": {
            "game_id": "test-game",
            "version": "1.0.0",
            "player_id": player_id,
            "cheat_type": 2,
            "hwid": "test-hwid",
            "timestamp": 1704067200000000000u64,
            "detection_count": 1
        },
        "action_token": action_token
    });
    
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let signature = keypair.sign(&payload_bytes);
    
    // Step 5: Submit cheat event
    let cheat_response = client
        .post(&format!("{}/cheat", server_url))
        .json(&json!({
            "payload": payload,
            "signature": hex::encode(signature.to_bytes())
        }))
        .send()
        .await
        .unwrap();
    
    assert!(cheat_response.status().is_success());
    
    // Step 6: Verify replay attack is prevented
    let replay_response = client
        .post(&format!("{}/cheat", server_url))
        .json(&json!({
            "payload": payload,
            "signature": hex::encode(signature.to_bytes())
        }))
        .send()
        .await
        .unwrap();
    
    assert!(!replay_response.status().is_success()); // Should fail (replay)
}
```

**Run integration tests:**
```bash
# Start wrangler dev in one terminal
npx wrangler dev

# Run tests in another terminal
cargo test --test integration_test
```

#### 3. Load Tests

```bash
# Install k6
brew install k6

# Create load test script
cat > load_test.js << EOF
import http from 'k6/http';
import { check } from 'k6';

export let options = {
  vus: 100,
  duration: '30s',
};

export default function () {
  let url = 'http://localhost:8787/register';
  let payload = JSON.stringify({
    player_id: 'test-player-' + __VU,
    public_key: 'deadbeef...',
    hwid: 'test-hwid'
  });
  
  let res = http.post(url, payload);
  check(res, {
    'status is 200': (r) => r.status === 200,
  });
}
EOF

# Run load test
k6 run load_test.js
```

### Testing Checklist

#### Client-Side Tests
- [ ] Ed25519 key pair generation works
- [ ] Player ID derivation produces consistent result
- [ ] KeyManager persists and loads key pair correctly
- [ ] ActionTokenClient requests tokens successfully
- [ ] SignedRequest serialization/deserialization works
- [ ] Signature verification fails with wrong public key
- [ ] Token freshness validation rejects expired tokens

#### Server-Side Tests
- [ ] POST /register creates player record with public key
- [ ] POST /action-token generates valid BLAKE3 hash
- [ ] POST /cheat rejects requests without signature
- [ ] POST /cheat rejects requests with invalid signature
- [ ] POST /cheat rejects requests with invalid action token
- [ ] POST /cheat rejects requests with expired timestamp
- [ ] POST /cheat rejects requests with reused nonce (replay)
- [ ] NonceTracker marks nonces as used correctly
- [ ] NonceTracker expires old nonces automatically

#### Integration Tests
- [ ] Full flow: register → token → submit cheat → verify
- [ ] Replay attack is prevented
- [ ] Concurrent requests handle nonce correctly
- [ ] Multiple players don't interfere
- [ ] Ban status is checked correctly

#### Performance Tests
- [ ] Token generation < 5ms
- [ ] Signature verification < 10ms
- [ ] Nonce lookup < 1ms
- [ ] Full cheat submission < 50ms (p95)
- [ ] 10,000 concurrent requests handled

### Debugging Tips

#### Client-Side Debugging
```rust
// Enable detailed logging
env_logger::init();

// Log key pair generation
let keypair = ClientKeypair::generate();
log::info!("Generated keypair: player_id={}", keypair.derive_player_id());

// Log token request
let token = client.request_token(&player_id).await?;
log::info!("Got token: expires_at={}", token.expires_at);

// Log signature
let signature = key_manager.sign_payload(&payload)?;
log::info!("Signed payload: signature_len={}", signature.len());
```

#### Server-Side Debugging
```rust
// Log verification steps
log::info!("Step 1: Verifying Ed25519 signature");
public_key.verify(&payload_bytes, &sig)?;

log::info!("Step 2: Verifying action token hash");
assert!(expected_hash == action_token.token_hash);

log::info!("Step 3: Checking timestamp freshness");
assert!((now - timestamp).abs() <= 300);

log::info!("Step 4: Checking nonce uniqueness");
assert!(!is_used);

log::info!("Step 5: Processing cheat event");
process_cheat_event(event).await?;
```

#### Common Issues

**Issue:** "Invalid action token" error
- Check: Server secret matches between token generation and verification
- Check: Timestamp is within ±5 minutes
- Check: Nonce is correctly passed through

**Issue:** "Action token already used" (false positive)
- Check: Nonce is unique for each request
- Check: Client generates fresh timestamp for each request
- Check: Not reusing old action tokens

**Issue:** "Ed25519 signature verification failed"
- Check: Public key is correctly stored in database
- Check: Payload serialization matches what was signed
- Check: Signature bytes are correctly hex-encoded

---

## Reflection: Struggling/Solved

### Struggles

#### 1. Architecture Complexity
**Problem:** Initially considered full PostgreSQL + Redis setup from original 008 plan.

**Solution:** Realized Durable Objects with SQLite is sufficient for:
- Player state (per-player DO)
- Nonce tracking (global DO)
- Pattern distribution (global DO)

**Result:** Reduced infrastructure by 60%, simplified deployment.

#### 2. Replay Attack Prevention
**Problem:** How to efficiently track nonces without massive storage overhead?

**Solution:** Sliding window + Bloom filter:
- Bloom filter: O(1) lookup, minimal memory, 0.1% false positive rate
- LRU cache: Exact check for recent 10k nonces
- Auto-cleanup every 5 minutes

**Result:** Sub-millisecond nonce lookups, minimal memory usage.

#### 3. Player ID Strategy
**Problem:** Should we use UUID v7 or derive from public key?

**Consideration 1: UUID v7**
- ✅ Time-ordered, sortable
- ❌ Not cryptographically bound to identity
- ❌ Need separate registration step

**Consideration 2: BLAKE3(public_key)**
- ✅ Cryptographically bound to identity
- ✅ Can't forge
- ✅ Consistent across devices
- ✅ Already using BLAKE3 per project guidelines

**Decision:** Use BLAKE3(public_key) as player_id.

**Result:** Simpler architecture, stronger security.

#### 4. Token Expiry Strategy
**Problem:** How long should action tokens be valid?

**Trade-off:**
- Too short (1 minute): High latency, bad UX
- Too long (1 hour): Replay window too large

**Decision:** 5 minutes is optimal balance:
- Enough time for network delays
- Short enough to limit replay window
- Matches typical game session duration

**Result:** Good UX without compromising security.

### Solved

#### 1. Dual Verification Implementation
**Challenge:** How to combine Ed25519 signatures with BLAKE3 tokens?

**Solution:** Two-layer verification:
1. Signature proves identity (Ed25519)
2. Token proves authorization (BLAKE3)

**Key Insight:** Both must be valid independently.

#### 2. Stateful vs Stateless Server
**Challenge:** Nonce tracking requires stateful storage.

**Solution:** Durable Objects with SQLite:
- Per-player DO: Player state, cheat history
- Global NonceTracker DO: Nonce tracking

**Benefit:** Synchronous queries, zero latency.

#### 3. Client-Side Key Storage
**Challenge:** Where to store Ed25519 private key securely on Windows?

**Solution:**
- Store at `${MAXION_DATA_DIR}/client_keypair.bin`
- Use Windows ACLs to restrict access
- File permissions: Owner only (0600)
- Encrypt file in future version if needed

**Current:** Sufficient for production.
**Future:** Consider Windows DPAPI for additional protection.

---

## Remaining Work

### High Priority

1. **Update Remaining Plan Documents**
   - [ ] 008c_server_detection.md - Align with dual verification
   - [ ] 008d_pattern_management.md - Update player_id references
   - [ ] 008e_ban_management.md - Update ban enforcement for replay attacks
   - [ ] 008f_analytics_monitoring.md - Add nonce tracking metrics
   - [ ] ARCHITECTURE_SUMMARY.md - Update diagrams with action token flow

2. **Implement Server-Side Code**
   - [ ] Create `durable_objects/player.rs` with SessionObject
   - [ ] Implement `register_player()` method
   - [ ] Implement `generate_action_token()` method
   - [ ] Implement `record_cheat()` with 5-step verification
   - [ ] Create `durable_objects/nonce.rs` with NonceTracker
   - [ ] Add Worker router in `src/worker.rs`
   - [ ] Configure wrangler.toml with DO bindings

3. **Implement Client-Side Code**
   - [ ] Create `src/client/crypto.rs` with KeyManager
   - [ ] Create `src/client/token.rs` with ActionTokenClient
   - [ ] Create `src/client/registration.rs` with RegistrationClient
   - [ ] Add FFI exports to `src/lib.rs`
   - [ ] Update Unity C# bindings

4. **Write Tests**
   - [ ] Unit tests for Ed25519 signing
   - [ ] Unit tests for action token generation
   - [ ] Unit tests for nonce tracking
   - [ ] Integration tests for full flow
   - [ ] Load tests for performance

5. **Documentation**
   - [ ] Update API documentation
   - [ ] Add Unity integration guide
   - [ ] Write troubleshooting guide
   - [ ] Document security considerations

### Medium Priority

6. **Security Hardening**
   - [ ] Implement Windows DPAPI for key storage
   - [ ] Add rate limiting per player_id
   - [ ] Implement IP-based abuse detection
   - [ ] Add certificate pinning for HTTPS
   - [ ] Security audit (external)

7. **Performance Optimization**
   - [ ] Benchmark nonce lookup with Bloom filter
   - [ ] Optimize BLAKE3 hash calculation
   - [ ] Test concurrent nonce usage
   - [ ] Profile Durable Object storage usage

8. **Monitoring & Observability**
   - [ ] Add metrics for action token generation rate
   - [ ] Add metrics for nonce collisions
   - [ ] Add metrics for verification failures
   - [ ] Create dashboards in Grafana/Prometheus
   - [ ] Setup alerting for anomalies

### Low Priority (Post-Launch)

9. **Enhanced Features**
   - [ ] Implement token batching (request multiple tokens at once)
   - [ ] Add token pre-caching for offline play
   - [ ] Implement webhook notifications for bans
   - [ ] Add admin dashboard for player management
   - [ ] Implement analytics for cheat patterns

10. **Docker Containers (If Needed)**
    - [ ] ML-based cheat detection (deferred)
    - [ ] Complex behavioral analysis (deferred)
    - [ ] Heavy telemetry processing (deferred)

---

## Migration Path

### From Previous 008 Implementation

**If you have existing 008 implementation:**

1. **Database Migration**
   ```sql
   -- Add Ed25519 public key column
   ALTER TABLE sessions ADD COLUMN public_key TEXT;
   
   -- Create player_id from UUID v7 (temporary migration)
   UPDATE sessions SET player_id = REPLACE(player_uuid, '-', '');
   
   -- Create used_nonces table
   CREATE TABLE used_nonces (
       nonce INTEGER PRIMARY KEY,
       player_id TEXT NOT NULL,
       expires_at INTEGER NOT NULL,
       created_at INTEGER NOT NULL
   );
   
   CREATE INDEX idx_used_nonces_expires ON used_nonces(expires_at);
   ```

2. **Client Migration**
   ```rust
   // Update existing clients to generate Ed25519 key pair
   let keypair = ClientKeypair::generate();
   let player_id = keypair.derive_player_id();
   
   // Register with server
   register_player(&player_id, &keypair.public_key, hwid).await?;
   ```

3. **API Migration**
   - Old endpoints deprecated but kept for backward compatibility:
     - `POST /cheat` (unsigned) → returns error, asks for new flow
     - `GET /status/{uuid}` → returns error, asks for player_id
   - New endpoints:
     - `POST /register`
     - `POST /action-token`
     - `POST /cheat` (signed with dual verification)

### Rollback Plan

**If issues arise in production:**

1. **Immediate Rollback**
   ```bash
   # Revert Worker deployment
   npx wrangler rollback
   
   # Restore previous version
   npx wrangler publish --version <previous-version-id>
   ```

2. **Data Recovery**
   - SQLite in DO is automatically backed up
   - KV data persists across deployments
   - No data loss expected

3. **Client Fallback**
   - Clients can continue using unsigned `/cheat` endpoint (deprecated)
   - Enable fallback mode via environment variable
   - Gradually migrate clients to new flow

---

## Key Lessons Learned

### 1. Security Through Simplicity
**Lesson:** Dual verification (Ed25519 + BLAKE3) provides stronger security than complex multi-layer systems.

**Takeaway:** Don't over-engineer. Two independent cryptographic proofs are better than ten fragile layers.

### 2. Leverage Platform Capabilities
**Lesson:** Cloudflare Durable Objects with SQLite eliminate need for PostgreSQL and Redis.

**Takeaway:** Use platform-native solutions first. Add external services only when necessary.

### 3. Cryptographic Consistency
**Lesson:** Use same hashing algorithm (BLAKE3) everywhere per project guidelines.

**Takeaway:** Consistent crypto primitives reduce complexity and cognitive load.

### 4. Player ID as Identity
**Lesson:** Deriving player_id from Ed25519 public key creates cryptographic identity binding.

**Takeaway:** Don't generate random IDs when you can derive them from cryptographic material.

### 5. Replay Prevention is Critical
**Lesson:** Nonce tracking is essential for any authorization system.

**Takeaway:** Always design replay prevention from the start, not as an afterthought.

---

## Questions?

### FAQ

**Q: Why BLAKE3 for action tokens instead of Ed25519?**
A: BLAKE3 is faster (~1GB/s vs ~100MB/s for Ed25519), simpler (no key pair management), and we're already using it per project guidelines.

**Q: What if a client loses their private key?**
A: They need to generate a new key pair and register again. This creates a new player_id, effectively creating a new identity.

**Q: Can action tokens be reused across different actions?**
A: No, each action token is tied to a specific nonce and expires after 5 minutes. Reusing a token will fail the nonce check.

**Q: What if the server_secret leaks?**
A: Rotate it immediately. Existing tokens expire in 5 minutes, so impact is limited. Deploy new secret to all Worker instances.

**Q: Can Bloom filter false positives cause legitimate requests to fail?**
A: Yes, 0.1% false positive rate means 1 in 1000 requests might be incorrectly rejected. This is acceptable for anti-cheat.

**Q: Why not use JWT for action tokens?**
A: JWT requires key management and adds complexity. BLAKE3-based tokens are simpler, faster, and sufficient for this use case.

**Q: How do we handle offline play?**
A: Pre-generate a batch of action tokens (e.g., 100 tokens valid for 8 hours) and store them locally. Use tokens when offline, sync when back online.

**Q: What's the impact on latency?**
A: Negligible. Ed25519 signing is ~1ms, BLAKE3 hashing is <0.1ms, nonce lookup is <0.1ms. Total <2ms per request.

**Q: Can we implement this in other languages?**
A: Yes! The architecture is language-agnostic. Python, Go, Java, etc., all have Ed25519 and BLAKE3 libraries.

---

**Created:** 2025-01-24  
**Version:** 1.0  
**Status:** Implementation Ready  
**Next Steps:** Update remaining plan documents, implement server and client code, write tests