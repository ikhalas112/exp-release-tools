# 008c: Server-Side Detection Service

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, Data Engineers, Security Analysts |

## Overview
This plan implements the core server-side detection service that processes telemetry from clients, analyzes behavioral patterns, detects anomalies, and generates threat scores. This component operates entirely server-side and requires connectivity to function.

## Architecture Notes

### Infrastructure Stack
- **Cloudflare Workers**: Request routing and dual verification (Ed25519 + BLAKE3)
- **Durable Objects**: Session state, player registration, nonce tracking
- **Durable Objects (SQLite)**: <1ms synchronous queries for session data
- **Cloudflare KV**: Player state cache and ban status lookup
- **Moka**: L1 cache for in-memory storage on Workers
- **External Systems**: PostgreSQL/Redis deferred to external analytics systems

### Detection Pipeline
```
Client Event (Ed25519 signed + BLAKE3 token)
  → Cloudflare Worker (5-Step Verification)
    → Verify Ed25519 signature
    → Verify BLAKE3 action token
    → Check timestamp (±5 min)
    → Check nonce uniqueness
    → Route to Durable Object
      → Session DO (SQLite)
        → Update player_state
        → Check ban status
        → Calculate threat score
      → Nonce Tracker DO
        → Mark nonce as used
        → Bloom filter + LRU cache
        → Auto-cleanup (5 min)
      → Cloudflare KV
        → Store player state
        → External systems read status
```

### Key Design Decisions

1. **Dual Verification Security**:
   - Ed25519 signatures prove client identity
   - BLAKE3 action tokens prove server authorization
   - Both must be valid independently
   - Replay attack prevention via nonce tracking

2. **Zero-Latency State Management**:
   - Durable Objects run SQLite in same thread
   - Synchronous queries (< 1ms latency)
   - No external database dependencies
   - Simple, cost-effective architecture

3. **Replay Attack Prevention**:
   - Nonce-based single-use tokens
   - Bloom filter for probabilistic check (0.1% false positive)
   - LRU cache for exact check (last 10,000 nonces)
   - 5-minute automatic cleanup window

4. **Stateless External Integration**:
   - External systems read from Cloudflare KV
   - No direct database access required
   - Simple HTTP API for queries

## Implementation Tasks

### Task 1: Detection Engine Setup (Day 1-2)

#### 1.1 Crate Structure
```
maxion-detection/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── mod.rs              # Module index
│   ├── types.rs            # Shared types
│   ├── worker.rs           # Cloudflare Worker entry point
│   ├── verification.rs    # Dual verification (Ed25519 + BLAKE3)
│   ├── durable_objects/
│   │   ├── mod.rs          # DO index
│   │   ├── player.rs       # Session DO (registration, tokens)
│   │   ├── nonce.rs        # Nonce Tracker DO (replay prevention)
│   │   └── telemetry.rs    # Telemetry DO (event aggregation)
│   ├── analyzer/
│   │   ├── mod.rs          # Analyzer index
│   │   ├── behavior.rs     # Behavioral analysis
│   │   ├── vpn.rs          # VPN/proxy detection
│   │   ├── fingerprint.rs  # OS fingerprinting
│   │   └── connection.rs   # Connection monitoring
│   ├── scoring.rs          # Threat scoring
│   ├── cache.rs            # L1 in-memory cache (Moka)
│   └── crypto.rs           # Ed25519 + BLAKE3 utilities
└── tests/
    ├── unit.rs
    └── integration.rs
```

#### 1.2 Cargo.toml
```toml
[package]
name = "maxion-detection"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.6", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Cryptography (Dual Verification)
ed25519-dalek = "2.1"
blake3 = "1.5"

# Cloudflare Workers
worker = "0.2"
worker-sys = "0.0"

# Async Runtime
tokio = { version = "1.35", features = ["full"] }
futures-util = "0.3"

# Caching (L1 only)
moka = { version = "0.12", features = ["future"] }

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tokio-test = "0.4"
```

### Task 2: Core Types (Day 2-3)

#### 2.1 Detection Types (types.rs)
```rust
// src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreatLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionContext {
 pub player_id: String,  // Derived from Ed25519 public_key
 pub session_id: String,
 pub timestamp: u64,
 pub ip_address: String,
 pub user_agent: String,
 pub signature: String,  // Ed25519 signature
 pub action_token: String,  // BLAKE3 action token
 pub nonce: String,  // Single-use nonce
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub threat_level: ThreatLevel,
    pub risk_score: f32,  // 0.0 to 1.0
    pub flags: Vec<DetectionFlag>,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionFlag {
    pub flag_type: FlagType,
    pub severity: ThreatLevel,
    pub confidence: f32,  // 0.0 to 1.0
    pub description: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlagType {
    VpnDetected,
    ProxyDetected,
    DatacenterIp,
    InconsistentFingerprint,
    SuspiciousBehavior,
    KnownCheatSignature,
    TimingAnomaly,
    ConnectionPattern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
 pub event_id: String,
 pub player_id: String,
 pub event_type: String,
 pub timestamp: u64,
 pub signature: String,  // Ed25519 signature
 pub action_token: String,  // BLAKE3 action token
 pub nonce: String,  // Single-use nonce
 pub data: HashMap<String, serde_json::Value>,
}
```

### Task 3: L1 In-Memory Cache (Day 3-4)

#### 3.1 Cache Implementation (cache.rs)
```rust
// src/cache.rs
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use anyhow::Result;

/// L1-only in-memory cache using Moka
/// 
/// Note: Durable Objects SQLite provides <1ms persistent storage,
/// so we only need in-memory caching for hot data on Workers.
/// External systems (analytics, monitoring) read from Cloudflare KV.
pub struct L1Cache<T>
where
    T: Clone + Send + Sync + 'static,
{
    cache: Cache<String, T>,
}

impl<T> L1Cache<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(max_capacity: u64, ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_secs))
            .time_to_idle(Duration::from_secs(ttl_secs / 2))
            .build();

        L1Cache { cache }
    }

    pub async fn get(&self, key: &str) -> Option<T> {
        self.cache.get(key).await
    }

    pub async fn set(&self, key: &str, value: T) {
        self.cache.insert(key.to_string(), value).await;
    }

    pub async fn invalidate(&self, key: &str) {
        self.cache.invalidate(key).await;
    }

    pub async fn invalidate_all(&self) {
        self.cache.invalidate_all();
    }

    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    pub fn weighted_size(&self) -> u64 {
        self.cache.weighted_size()
    }
}

/// Common cache configurations
pub mod config {
    /// Player public key cache: 10,000 entries, 1 hour TTL
    pub fn player_key_cache() -> super::L1Cache<Vec<u8>> {
        super::L1Cache::new(10_000, 3600)
    }

    /// Action token cache: 5,000 entries, 5 minute TTL
    pub fn action_token_cache() -> super::L1Cache<String> {
        super::L1Cache::new(5_000, 300)
    }

    /// Ban status cache: 50,000 entries, 10 minute TTL
    pub fn ban_status_cache() -> super::L1Cache<String> {
        super::L1Cache::new(50_000, 600)
    }
}
```

### Task 4: VPN/Proxy Detection (Day 4-6)

#### 4.1 Detection Strategies
- IP reputation database lookup
- Check against known VPN/proxy IP ranges
- Datacenter IP detection (AWS, GCP, Azure, etc.)
- Connection timing analysis (characteristic of proxies)
- User-Agent consistency checks

#### 4.2 Implementation (analyzer/vpn.rs)
```rust
// src/analyzer/vpn.rs
use crate::types::{DetectionContext, DetectionFlag, FlagType, ThreatLevel};
use crate::cache::MultiTierCache;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpReputation {
 pub is_vpn: bool,
 pub is_proxy: bool,
 pub is_datacenter: bool,
 pub is_hosting: bool,
 pub risk_score: f32,
 pub provider: Option<String>,
 pub cached_at: u64,
}

impl IpReputation {
    pub fn safe() -> Self {
        IpReputation {
            is_vpn: false,
            is_proxy: false,
            is_datacenter: false,
            is_hosting: false,
            risk_score: 0.0,
            provider: None,
        }
    }
}

pub struct VpnDetector {
    reputation_cache: MultiTierCache<IpReputation>,
    vpn_ip_ranges: HashSet<String>,
    proxy_ip_ranges: HashSet<String>,
    datacenter_asns: HashSet<u32>,
}

impl VpnDetector {
    pub fn new(redis_url: &str) -> Result<Self> {
        let reputation_cache = MultiTierCache::new(10000, redis_url, 3600)?;

        Ok(VpnDetector {
            reputation_cache,
            vpn_ip_ranges: HashSet::new(),
            proxy_ip_ranges: HashSet::new(),
            datacenter_asns: HashSet::new(),
        })
    }

    pub async fn detect(
        &self,
        context: &DetectionContext,
    ) -> Result<Vec<DetectionFlag>> {
        let mut flags = Vec::new();

        // Check IP reputation
        let reputation = self.get_ip_reputation(&context.ip_address).await?;

        if reputation.is_vpn {
            flags.push(DetectionFlag {
                flag_type: FlagType::VpnDetected,
                severity: ThreatLevel::High,
                confidence: 0.85,
                description: "VPN connection detected".to_string(),
                evidence: format!("IP: {}", context.ip_address),
            });
        }

        if reputation.is_proxy {
            flags.push(DetectionFlag {
                flag_type: FlagType::ProxyDetected,
                severity: ThreatLevel::Medium,
                confidence: 0.75,
                description: "Proxy connection detected".to_string(),
                evidence: format!("IP: {}", context.ip_address),
            });
        }

        if reputation.is_datacenter {
            flags.push(DetectionFlag {
                flag_type: FlagType::DatacenterIp,
                severity: ThreatLevel::Medium,
                confidence: 0.90,
                description: "Datacenter IP detected".to_string(),
                evidence: format!("IP: {}", context.ip_address),
            });
        }

        Ok(flags)
    }

    async fn get_ip_reputation(&self, ip: &str) -> Result<IpReputation> {
        // Check cache first
        if let Some(reputation) = self.reputation_cache.get(ip).await {
            return Ok(reputation);
        }

        // In production, this would query an IP intelligence service
        // For now, return safe reputation
        let reputation = IpReputation::safe();
        self.reputation_cache.set(ip, &reputation).await?;

        Ok(reputation)
    }
}
```

### Task 5: OS Fingerprinting (Day 6-8)

#### 5.1 Fingerprinting Strategy
- Hardware ID consistency analysis
- System information validation
- Browser/User-Agent fingerprinting
- Canvas fingerprinting (if applicable)
- Timing-based fingerprinting

#### 5.2 Implementation (analyzer/fingerprint.rs)
```rust
// src/analyzer/fingerprint.rs
use crate::types::{DetectionContext, DetectionFlag, FlagType, ThreatLevel};
use crate::cache::MultiTierCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintData {
    pub hardware_id: String,
    pub os_version: String,
    pub cpu_info: String,
    pub gpu_info: String,
    pub ram_info: String,
    pub screen_resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintHistory {
    pub player_id: String,
    pub fingerprints: Vec<FingerprintEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintEntry {
    pub timestamp: u64,
    pub fingerprint: String,
    pub ip_address: String,
}

pub struct FingerprintAnalyzer {
    history_cache: MultiTierCache<FingerprintHistory>,
    tolerance_threshold: f32,
}

impl FingerprintAnalyzer {
    pub fn new(redis_url: &str) -> Result<Self> {
        let history_cache = MultiTierCache::new(10000, redis_url, 86400)?;

        Ok(FingerprintAnalyzer {
            history_cache,
            tolerance_threshold: 0.8,
        })
    }

    pub async fn analyze(
        &self,
        context: &DetectionContext,
        current_fingerprint: &FingerprintData,
    ) -> Result<Vec<DetectionFlag>> {
        let mut flags = Vec::new();

        // Generate fingerprint hash
        let fingerprint_hash = self.generate_fingerprint_hash(current_fingerprint);

        // Get historical fingerprints
        let history = self.get_fingerprint_history(&context.player_id).await?;

        // Compare with historical data
        if !history.fingerprints.is_empty() {
            let similarity = self.calculate_similarity(&fingerprint_hash, &history);

            if similarity < self.tolerance_threshold {
                flags.push(DetectionFlag {
                    flag_type: FlagType::InconsistentFingerprint,
                    severity: ThreatLevel::High,
                    confidence: 1.0 - similarity,
                    description: "System fingerprint inconsistency detected".to_string(),
                    evidence: format!("Similarity: {:.2}", similarity),
                });
            }
        }

        // Update history
        self.update_fingerprint_history(context, &fingerprint_hash).await?;

        Ok(flags)
    }

    fn generate_fingerprint_hash(&self, data: &FingerprintData) -> String {
        use blake3::Hash;
        let input = format!(
            "{}|{}|{}|{}|{}|{}",
            data.hardware_id,
            data.os_version,
            data.cpu_info,
            data.gpu_info,
            data.ram_info,
            data.screen_resolution
        );
        format!("{:x}", Hash::hash(input.as_bytes()))
    }

    fn calculate_similarity(&self, current: &str, history: &FingerprintHistory) -> f32 {
        // Calculate similarity with most recent fingerprints
        let recent: Vec<_> = history.fingerprints
            .iter()
            .take(5)  // Last 5 fingerprints
            .collect();

        let total_similarity: f32 = recent
            .iter()
            .map(|entry| self.string_similarity(current, &entry.fingerprint))
            .sum();

        total_similarity / recent.len() as f32
    }

    fn string_similarity(&self, a: &str, b: &str) -> f32 {
        // Simple Hamming distance for similarity
        let min_len = a.len().min(b.len());
        let max_len = a.len().max(b.len());

        if max_len == 0 {
            return 1.0;
        }

        let differences = a.chars()
            .zip(b.chars())
            .filter(|(a, b)| a != b)
            .count();

        1.0 - (differences as f32 / max_len as f32)
    }

    async fn get_fingerprint_history(&self, player_id: &str) -> Result<FingerprintHistory> {
        match self.history_cache.get(player_id).await {
            Some(history) => Ok(history),
            None => Ok(FingerprintHistory {
                player_id: player_id.to_string(),
                fingerprints: Vec::new(),
            }),
        }
    }

    async fn update_fingerprint_history(
        &self,
        context: &DetectionContext,
        fingerprint_hash: &str,
    ) -> Result<()> {
        let mut history = self.get_fingerprint_history(&context.player_id).await?;

        history.fingerprints.push(FingerprintEntry {
            timestamp: context.timestamp,
            fingerprint: fingerprint_hash.to_string(),
            ip_address: context.ip_address.clone(),
        });

        // Keep only last 50 entries
        if history.fingerprints.len() > 50 {
            history.fingerprints.remove(0);
        }

        self.history_cache.set(&context.player_id, &history).await?;

        Ok(())
    }
}
```

### Task 6: Behavioral Analysis (Day 8-10)

#### 6.1 Analysis Dimensions
- Input timing and frequency (from client telemetry)
- Movement patterns (position, velocity, acceleration)
- Interaction patterns (clicks, key presses)
- Reaction times
- Macro detection (statistical analysis)

#### 6.2 Implementation (analyzer/behavior.rs)
```rust
// src/analyzer/behavior.rs
use crate::types::{DetectionContext, DetectionFlag, FlagType, ThreatLevel, TelemetryEvent};
use std::collections::VecDeque;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct InputTimingData {
    pub timestamp: u64,
    pub interval_ms: u64,
}

#[derive(Debug, Clone)]
pub struct MovementData {
    pub timestamp: u64,
    pub position_x: f32,
    pub position_y: f32,
    pub velocity: f32,
    pub acceleration: f32,
}

pub struct BehaviorAnalyzer {
    input_buffer: VecDeque<InputTimingData>,
    movement_buffer: VecDeque<MovementData>,
    buffer_size: usize,
}

impl BehaviorAnalyzer {
    pub fn new() -> Self {
        BehaviorAnalyzer {
            input_buffer: VecDeque::with_capacity(100),
            movement_buffer: VecDeque::with_capacity(100),
            buffer_size: 100,
        }
    }

    pub fn analyze_events(
        &mut self,
        context: &DetectionContext,
        events: &[TelemetryEvent],
    ) -> Result<Vec<DetectionFlag>> {
        let mut flags = Vec::new();

        for event in events {
            match event.event_type.as_str() {
                "input" => {
                    if let Some(input_flags) = self.analyze_input_timing(event)? {
                        flags.extend(input_flags);
                    }
                }
                "movement" => {
                    if let Some(movement_flags) = self.analyze_movement(event)? {
                        flags.extend(movement_flags);
                    }
                }
                _ => {}
            }
        }

        Ok(flags)
    }

    fn analyze_input_timing(&mut self, event: &TelemetryEvent) -> Result<Option<Vec<DetectionFlag>>> {
        let timestamp = event.timestamp;
        let interval_ms = extract_interval(&event.data)?;

        self.input_buffer.push_back(InputTimingData {
            timestamp,
            interval_ms,
        });

        if self.input_buffer.len() > self.buffer_size {
            self.input_buffer.pop_front();
        }

        if self.input_buffer.len() < 10 {
            return Ok(None);
        }

        // Calculate jitter and variance
        let intervals: Vec<u64> = self.input_buffer.iter()
            .map(|d| d.interval_ms)
            .collect();

        let mean = calculate_mean(&intervals);
        let variance = calculate_variance(&intervals, mean);
        let jitter = variance.sqrt();

        // Detect automated input (very low jitter)
        if jitter < 0.5 {
            return Ok(Some(vec![DetectionFlag {
                flag_type: FlagType::TimingAnomaly,
                severity: ThreatLevel::High,
                confidence: 0.85,
                description: "Automated input detected (low timing jitter)".to_string(),
                evidence: format!("Jitter: {:.2}ms", jitter),
            }]));
        }

        // Detect macro patterns (repeated intervals)
        if self.detect_repeated_intervals(&intervals) {
            return Ok(Some(vec![DetectionFlag {
                flag_type: FlagType::SuspiciousBehavior,
                severity: ThreatLevel::Medium,
                confidence: 0.70,
                description: "Macro pattern detected".to_string(),
                evidence: "Repeated timing pattern".to_string(),
            }]));
        }

        Ok(None)
    }

    fn analyze_movement(&mut self, event: &TelemetryEvent) -> Result<Option<Vec<DetectionFlag>>> {
        let timestamp = event.timestamp;
        let (position_x, position_y) = extract_position(&event.data)?;

        // Calculate velocity and acceleration
        let velocity = if let Some(last) = self.movement_buffer.back() {
            let dx = position_x - last.position_x;
            let dy = position_y - last.position_y;
            let dt = timestamp - last.timestamp;
            (dx * dx + dy * dy).sqrt() / dt as f32
        } else {
            0.0
        };

        let acceleration = if let Some(last) = self.movement_buffer.back() {
            (velocity - last.velocity) / (timestamp - last.timestamp) as f32
        } else {
            0.0
        };

        self.movement_buffer.push_back(MovementData {
            timestamp,
            position_x,
            position_y,
            velocity,
            acceleration,
        });

        if self.movement_buffer.len() > self.buffer_size {
            self.movement_buffer.pop_front();
        }

        if self.movement_buffer.len() < 5 {
            return Ok(None);
        }

        // Detect impossible acceleration
        if acceleration.abs() > 1000.0 {
            return Ok(Some(vec![DetectionFlag {
                flag_type: FlagType::SuspiciousBehavior,
                severity: ThreatLevel::Critical,
                confidence: 0.90,
                description: "Impossible acceleration detected".to_string(),
                evidence: format!("Acceleration: {:.2}", acceleration),
            }]));
        }

        Ok(None)
    }

    fn detect_repeated_intervals(&self, intervals: &[u64]) -> bool {
        // Check for repeated patterns
        let mut pattern_count = 0;
        let threshold = 3;  // Allow 3ms variation

        for window in intervals.windows(5) {
            let first = window[0];
            let all_similar = window.iter().all(|&x| (x as i64 - first as i64).abs() <= threshold);

            if all_similar {
                pattern_count += 1;
            }
        }

        pattern_count > intervals.len() / 3
    }
}

fn calculate_mean(values: &[u64]) -> f64 {
    let sum: u64 = values.iter().sum();
    sum as f64 / values.len() as f64
}

fn calculate_variance(values: &[u64], mean: f64) -> f64 {
    let sum_squared_diff: f64 = values.iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum();

    sum_squared_diff / values.len() as f64
}

fn extract_interval(data: &serde_json::Value) -> Result<u64> {
    Ok(data["interval_ms"].as_u64().unwrap_or(0))
}

fn extract_position(data: &serde_json::Value) -> Result<(f32, f32)> {
    Ok((
        data["x"].as_f64().unwrap_or(0.0) as f32,
        data["y"].as_f64().unwrap_or(0.0) as f32,
    ))
}
```

### Task 7: Connection Monitoring (Day 10-11)

#### 7.1 Monitoring Metrics
- Connection frequency
- IP changes during session
- Connection timing patterns
- Protocol anomalies

#### 7.2 Implementation (analyzer/connection.rs)
```rust
// src/analyzer/connection.rs
use crate::types::{DetectionContext, DetectionFlag, FlagType, ThreatLevel};
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ConnectionRecord {
    pub timestamp: u64,
    pub ip_address: String,
    pub user_agent: String,
}

pub struct ConnectionMonitor {
    player_connections: HashMap<String, Vec<ConnectionRecord>>,
    ip_change_threshold: u64,  // seconds
}

impl ConnectionMonitor {
    pub fn new() -> Self {
        ConnectionMonitor {
            player_connections: HashMap::new(),
            ip_change_threshold: 300,  // 5 minutes
        }
    }

    pub async fn monitor_connection(
        &mut self,
        context: &DetectionContext,
    ) -> Result<Vec<DetectionFlag>> {
        let mut flags = Vec::new();

        let player_id = &context.player_id;
        let connections = self.player_connections.entry(player_id.clone())
            .or_insert_with(Vec::new);

        // Check for IP changes
        if !connections.is_empty() {
            let last_ip = connections.last().unwrap().ip_address.clone();
            let last_timestamp = connections.last().unwrap().timestamp;

            if last_ip != context.ip_address {
                let time_since_last = context.timestamp - last_timestamp;

                if time_since_last < self.ip_change_threshold {
                    flags.push(DetectionFlag {
                        flag_type: FlagType::ConnectionPattern,
                        severity: ThreatLevel::High,
                        confidence: 0.80,
                        description: "Rapid IP change detected".to_string(),
                        evidence: format!(
                            "Changed from {} to {} in {}s",
                            last_ip, context.ip_address, time_since_last
                        ),
                    });
                }
            }
        }

        // Record connection
        connections.push(ConnectionRecord {
            timestamp: context.timestamp,
            ip_address: context.ip_address.clone(),
            user_agent: context.user_agent.clone(),
        });

        // Keep only last 100 connections
        if connections.len() > 100 {
            connections.remove(0);
        }

        // Check for connection flooding
        if self.detect_connection_flood(connections) {
            flags.push(DetectionFlag {
                flag_type: FlagType::ConnectionPattern,
                severity: ThreatLevel::Medium,
                confidence: 0.70,
                description: "Connection flooding detected".to_string(),
                evidence: "Multiple rapid connections".to_string(),
            });
        }

        Ok(flags)
    }

    fn detect_connection_flood(&self, connections: &[ConnectionRecord]) -> bool {
        if connections.len() < 10 {
            return false;
        }

        // Check for 10 connections in 1 minute
        let recent: Vec<_> = connections.iter()
            .filter(|c| {
                let now = Utc::now().timestamp() as u64;
                now - c.timestamp < 60
            })
            .collect();

        recent.len() >= 10
    }
}
```

### Task 8: Threat Scoring (Day 11-13)

#### 8.1 Scoring Algorithm
```rust
// src/scoring.rs
use crate::types::{DetectionFlag, DetectionResult, ThreatLevel};
use std::collections::HashMap;

pub struct ThreatScorer {
    severity_weights: HashMap<ThreatLevel, f32>,
    base_risk_increment: f32,
}

impl ThreatScorer {
    pub fn new() -> Self {
        let mut severity_weights = HashMap::new();
        severity_weights.insert(ThreatLevel::Safe, 0.0);
        severity_weights.insert(ThreatLevel::Low, 0.1);
        severity_weights.insert(ThreatLevel::Medium, 0.3);
        severity_weights.insert(ThreatLevel::High, 0.6);
        severity_weights.insert(ThreatLevel::Critical, 0.9);

        ThreatScorer {
            severity_weights,
            base_risk_increment: 0.05,
        }
    }

    pub fn calculate_score(&self, flags: &[DetectionFlag]) -> DetectionResult {
        if flags.is_empty() {
            return DetectionResult {
                threat_level: ThreatLevel::Safe,
                risk_score: 0.0,
                flags: Vec::new(),
                details: HashMap::new(),
            };
        }

        let mut risk_score = 0.0;
        let mut max_severity = ThreatLevel::Low;
        let mut details = HashMap::new();

        for flag in flags {
            // Weight risk by severity and confidence
            let severity_weight = self.severity_weights.get(&flag.severity)
                .unwrap_or(&0.0);
            let weighted_risk = severity_weight * flag.confidence;
            risk_score += weighted_risk;

            // Update max severity
            if flag.severity as u8 > max_severity as u8 {
                max_severity = flag.severity.clone();
            }

            // Add to details
            details.insert(
                format!("{:?}", flag.flag_type),
                format!("{} (confidence: {:.2})", flag.description, flag.confidence),
            );
        }

        // Cap risk score at 1.0
        risk_score = risk_score.min(1.0);

        // Determine threat level based on risk score
        let threat_level = match risk_score {
            x if x < 0.1 => ThreatLevel::Safe,
            x if x < 0.3 => ThreatLevel::Low,
            x if x < 0.5 => ThreatLevel::Medium,
            x if x < 0.8 => ThreatLevel::High,
            _ => ThreatLevel::Critical,
        };

        DetectionResult {
            threat_level,
            risk_score,
            flags: flags.to_vec(),
            details,
        }
    }
}
```

### Task 9: Main Detection Engine (Day 13-14)

#### 9.1 Engine Orchestration (engine.rs)
```rust
// src/engine.rs
use crate::types::{DetectionContext, DetectionResult, FingerprintData, TelemetryEvent};
use crate::analyzer::{VpnDetector, FingerprintAnalyzer, BehaviorAnalyzer, ConnectionMonitor};
use crate::scoring::ThreatScorer;
use anyhow::Result;

pub struct DetectionEngine {
    vpn_detector: VpnDetector,
    fingerprint_analyzer: FingerprintAnalyzer,
    behavior_analyzer: BehaviorAnalyzer,
    connection_monitor: ConnectionMonitor,
    threat_scorer: ThreatScorer,
}

impl DetectionEngine {
    pub fn new(redis_url: &str) -> Result<Self> {
        Ok(DetectionEngine {
            vpn_detector: VpnDetector::new(redis_url)?,
            fingerprint_analyzer: FingerprintAnalyzer::new(redis_url)?,
            behavior_analyzer: BehaviorAnalyzer::new(),
            connection_monitor: ConnectionMonitor::new(),
            threat_scorer: ThreatScorer::new(),
        })
    }

    pub async fn analyze_session(
        &mut self,
        context: &DetectionContext,
        fingerprint: Option<&FingerprintData>,
        events: &[TelemetryEvent],
    ) -> Result<DetectionResult> {
        let mut all_flags = Vec::new();

        // Run all analyzers
        all_flags.extend(self.vpn_detector.detect(context).await?);

        all_flags.extend(self.connection_monitor.monitor_connection(context).await?);

        if let Some(fp_data) = fingerprint {
            all_flags.extend(self.fingerprint_analyzer.analyze(context, fp_data).await?);
        }

        if !events.is_empty() {
            all_flags.extend(self.behavior_analyzer.analyze_events(context, events)?);
        }

        // Calculate final threat score
        let result = self.threat_scorer.calculate_score(&all_flags);

        Ok(result)
    }
}
```

### Task 10: API Integration (Day 14-15)

#### 10.1 Axum Endpoints (main.rs)
```rust
// src/main.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::init();

    let database_url = std::env::var("DATABASE_URL")?;
    let redis_url = std::env::var("REDIS_URL")?;

    let db_pool = PgPool::connect(&database_url).await?;
    let engine = DetectionEngine::new(&redis_url)?;

    let app = axum::Router::new()
        .route("/api/v1/detect", axum::routing::post(detect_threat))
        .route("/api/v1/analyze/:player_id", axum::routing::get(get_analysis))
        .layer(CorsLayer::permissive())
        .with_state(AppState { engine, db: db_pool });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Clone)]
struct AppState {
    engine: DetectionEngine,
    db: PgPool,
}

async fn detect_threat(
    State(state): State<AppState>,
    Json(request): Json<DetectionRequest>,
) -> Result<Json<DetectionResult>, ApiError> {
    let mut engine = state.engine;

    let result = engine.analyze_session(
        &request.context,
        request.fingerprint.as_ref(),
        &request.events,
    ).await?;

    // Store result in database
    store_detection_result(&state.db, &request.context.player_id, &result).await?;

    Ok(Json(result))
}

#[derive(serde::Deserialize)]
struct DetectionRequest {
    context: DetectionContext,
    fingerprint: Option<FingerprintData>,
    events: Vec<TelemetryEvent>,
}

async fn store_detection_result(
    pool: &PgPool,
    player_id: &str,
    result: &DetectionResult,
) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(&format!(
        "INSERT INTO detection_results (player_id, threat_level, risk_score, timestamp) \
         VALUES ('{}', '{}', {}, {})",
        player_id,
        format!("{:?}", result.threat_level),
        result.risk_score,
        chrono::Utc::now().timestamp()
    ))
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Detection error: {0}")]
    Detection(#[from] anyhow::Error),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Detection(_) => StatusCode::BAD_REQUEST,
        };

        (status, format!("{}", self)).into_response()
    }
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_scoring() {
        let scorer = ThreatScorer::new();
        let flags = vec![
            DetectionFlag {
                flag_type: FlagType::VpnDetected,
                severity: ThreatLevel::High,
                confidence: 0.9,
                description: "Test".to_string(),
                evidence: "Test".to_string(),
            },
        ];

        let result = scorer.calculate_score(&flags);
        assert_eq!(result.threat_level, ThreatLevel::High);
        assert!(result.risk_score > 0.5);
    }

    #[test]
    fn test_input_timing_analysis() {
        let mut analyzer = BehaviorAnalyzer::new();
        let event = create_test_input_event();
        let flags = analyzer.analyze_input_timing(&event).unwrap();

        // Verify detection logic
    }

    #[tokio::test]
    async fn test_vpn_detection() {
        let detector = VpnDetector::new("redis://localhost").unwrap();
        let context = create_test_context();
        let flags = detector.detect(&context).await.unwrap();

        // Verify VPN detection
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_full_detection_pipeline() {
    let engine = DetectionEngine::new("redis://localhost").unwrap();
    let context = create_test_context();
    let fingerprint = create_test_fingerprint();
    let events = create_test_events();

    let result = engine.analyze_session(&context, Some(&fingerprint), &events).await.unwrap();

    // Verify comprehensive detection
    assert!(!result.flags.is_empty() || result.risk_score == 0.0);
}
```

### Performance Tests
- Process 10,000 events/second
- Sub-100ms detection latency
- Cache hit rate > 90%

## Performance Requirements

- **Detection Latency**: < 100ms per analysis
- **Throughput**: 10,000 detections/second
- **Cache Hit Rate**: > 90%
- **Memory Usage**: < 1GB per instance
- **Database Load**: < 1000 queries/second

## Security Considerations

### Data Protection
- Encrypt sensitive telemetry
- Anonymize IP addresses in logs
- Retain data for limited time (GDPR)

### Anti-Abuse
- Rate limit API endpoints
- Detect and block detection evasion attempts
- Monitor for system abuse

### Compliance
- GDPR compliance for data collection
- Right to be forgotten implementation
- Audit logging for all detections

## Dependencies

### Core
- `sqlx` - Database driver (use raw_sql)
- `redis` - L2 cache
- `moka` - L1 cache
- `axum` - Web framework
- `tokio` - Async runtime

### Security
- `blake3` - Hashing (per project guidelines)
- `argon2` - Password hashing
- `jsonwebtoken` - JWT tokens

### Testing
- `mockall` - Mocking
- `tokio-test` - Async testing

## Deliverables

1. ✅ Complete detection engine
2. ✅ VPN/proxy detection
3. ✅ OS fingerprinting
4. ✅ Behavioral analysis
5. ✅ Connection monitoring
6. ✅ Threat scoring system
7. ✅ Multi-tier caching
8. ✅ API integration
9. ✅ Unit and integration tests
10. ✅ Performance benchmarks
11. ✅ Documentation

## Next Steps

After completing this phase, proceed to:
- **008d**: Pattern Management System
- **008e**: Ban Management Service
- **008f**: Analytics & Monitoring

## Notes

- Follow project coding style: snake_case, match over if, early returns
- Use `sqlx::raw_sql` for all database queries (per project guidelines)
- Use `blake3` for all hashing operations
- Cache invalidation strategy must be robust
- All timestamps use Unix timestamps (u64)
- Error messages should not leak internal details
- Log all detections for audit trail
- Implement graceful degradation if cache fails

## Known Limitations

- IP reputation database requires external service integration
- Behavioral analysis may have false positives
- Fingerprinting can be evaded by sophisticated attackers
- Cache coherency across multiple instances

## Migration Path

### From 008b to 008c
1. Integrate telemetry ingestion
2. Connect WebSocket to detection engine
3. Implement real-time threat notifications

### To 008d
1. Subscribe to pattern updates
2. Implement pattern-based detection
3. Add pattern distribution hooks