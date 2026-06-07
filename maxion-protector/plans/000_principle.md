# MMORPG Development Principles & Best Practices

## Overview

This document defines the architectural principles and coding standards that MUST be followed when designing and implementing MMORPG systems. All plans MUST be reviewed against these principles before implementation begins.

## Core Philosophy

> **"Don't Trust, Verify"** - Every design decision must be scrutinized against these principles. Assumptions without verification are the root cause of production failures.

## Mandatory Review Checklist

Before approving ANY plan, verify:

- [ ] Single Source of Truth
- [ ] Generic & Composable
- [ ] Production Grade
- [ ] **SOLID**: Single responsibility, Open/closed, Liskov substitution, Interface segregation, Dependency inversion
- [ ] **DRY**: No code duplication, extract shared types and behaviors
- [ ] **Scalable**: Horizontal scaling strategy, no single points of failure
- [ ] **Atomic**: Lock-free operations for hot paths, no RwLock contention
- [ ] **Zero-Copy**: Minimize cloning, use references and shared ownership
- [ ] **Performance**: Batching, caching, benchmarking validated
- [ ] **Simring (io_uring) Compatible**: Integrated with `mmorpg-sync` prediction system
- [ ] **WebTransport Compatible**: Aligned with `mmorpg-webtransport` protocol
- [ ] **DashMap**: Use as a direct replacement for `RwLock<HashMap<K, V>>`
- [ ] **Concurrency**: No thread/task leaks, scoped concurrency, Drop-based cleanup, no Arc cycles

---

## 1. SOLID Principles for MMORPG Architecture

### Single Responsibility Principle (SRP)

**❌ VIOLATION:**
```rust
pub struct QuestManager {
    quest_templates: Arc<RwLock<HashMap<Uuid, QuestTemplate>>>,
    quest_instances: Arc<RwLock<HashMap<Uuid, QuestInstance>>>,
    player_quests: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    entity_manager: Arc<EntityManager>,
    event_rx: mpsc::Receiver<QuestEvent>,
    // Does: loading, instance mgmt, events, rewards, validation
}
```

**✅ CORRECT:**
```rust
// Split into focused components:
pub struct QuestLoader;           // Load templates from DB
pub struct QuestInstanceManager;   // Manage instance lifecycle
pub struct QuestEventProcessor;    // Handle game events
pub struct QuestRewardDistributor; // Calculate and distribute rewards
pub struct QuestValidator;         // Enforce quest rules
```

**Rule:** Each struct should have ONE reason to change. If you can describe it as "XManager", it's likely doing too much.

### Open/Closed Principle (OCP)

**❌ VIOLATION:**
```rust
pub fn process_quest_completion(&self, quest: &QuestInstance) {
    match quest.quest_type {
        QuestType::MainStory => { /* hardcoded logic */ }
        QuestType::SideQuest => { /* hardcoded logic */ }
        QuestType::Daily => { /* hardcoded logic */ }
    }
}
```

**✅ CORRECT:**
```rust
pub trait QuestCompletionHandler: Send + Sync {
    fn handle_completion(&self, quest: &QuestInstance, player: &Player) -> Result<Reward>;
}

pub struct MainStoryHandler;
impl QuestCompletionHandler for MainStoryHandler { /* ... */ }

pub struct QuestCompleter {
    handlers: HashMap<QuestType, Box<dyn QuestCompletionHandler>>,
}
```

**Rule:** Use traits and composition over enum matching for extensible behavior.

### Liskov Substitution Principle (LSP)

**Rule:** All implementations of a trait must be interchangeable without breaking behavior.

### Interface Segregation Principle (ISP)

**❌ VIOLATION:**
```rust
pub trait GameManager {
    fn handle_combat(&self);
    fn handle_quests(&self);
    fn handle_economy(&self);
    fn handle_chat(&self);
    fn handle_pvp(&self);
}
```

**✅ CORRECT:**
```rust
pub trait CombatManager { fn handle_combat(&self); }
pub trait QuestManager { fn handle_quests(&self); }
pub trait EconomyManager { fn handle_economy(&self); }
```

**Rule:** Clients shouldn't depend on interfaces they don't use.

### Dependency Inversion Principle (DIP)

**❌ VIOLATION:**
```rust
pub struct BossManager {
    postgres: PgPool,  // Concrete dependency
    redis: RedisPool,  // Concrete dependency
}
```

**✅ CORRECT:**
```rust
pub trait BossRepository: Send + Sync {
    async fn save_instance(&self, instance: &BossInstance) -> Result<()>;
    async fn load_instances(&self) -> Result<Vec<BossInstance>>;
}

pub struct BossManager<R: BossRepository> {
    repository: Arc<R>,
}
```

**Rule:** Depend on abstractions, not concretions.

---

## 2. DRY: Don't Repeat Yourself

### Shared Combat System

**❌ VIOLATION:** Three different damage tracking implementations
```rust
// Boss system:
pub struct BossDamageStats {
    pub total_damage: u64,
    pub highest_single_hit: u32,
    pub damage_breakdown: HashMap<DamageType, u64>,
}

// Party system:
pub struct PartyMember {
    pub damage_contributed_total: u64,
    pub damage_contributed_session: u64,
}

// PvP system:
pub struct PvPParticipant {
    pub damage_dealt: u64,
    pub damage_taken: u64,
}
```

**✅ CORRECT:** Extract to shared combat crate
```rust
// crates/mmorpg-combat/src/tracker.rs
pub trait DamageTracker: Send + Sync {
    fn record_damage(&mut self, source: Uuid, amount: u64, damage_type: DamageType);
    fn get_total_damage(&self, source: Uuid) -> u64;
    fn get_damage_by_type(&self, source: Uuid, damage_type: DamageType) -> u64;
}

pub struct CombatDamageTracker {
    damage: DashMap<Uuid, Arc<RwLock<PlayerDamageStats>>>,
}

pub struct PlayerDamageStats {
    pub total: AtomicU64,
    pub session: AtomicU64,
    pub by_type: HashMap<DamageType, AtomicU64>,
    pub highest_single_hit: AtomicU32,
}
```

### Shared Loot Distribution

**❌ VIOLATION:** Duplicated in Boss and Party systems
```rust
// Both systems define identical enum:
pub enum LootDistributionMethod {
    FreeForAll,
    Roll,
    MasterLoot,
    NeedBeforeGreed,
    RoundRobin,
}
```

**✅ CORRECT:** Extract to shared loot crate
```rust
// crates/mmorpg-loot/src/distribution.rs
pub trait LootDistributor: Send + Sync {
    fn distribute(
        &self,
        items: Vec<LootItem>,
        participants: &[Uuid],
        method: LootDistributionMethod,
        context: LootContext,
    ) -> HashMap<Uuid, Vec<LootItem>>;
}

pub enum LootDistributionMethod {
    FreeForAll,
    Roll { timeout_seconds: u32 },
    MasterLoot { leader_id: Uuid },
    NeedBeforeGreed { timeout_seconds: u32 },
    RoundRobin { start_index: usize },
    MostDamage,
    LastHit,
}

pub struct LootContext {
    pub damage_contributions: HashMap<Uuid, u64>,
    pub last_hitter: Option<Uuid>,
    pub party_id: Option<Uuid>,
}
```

### Shared Event System

**❌ VIOLATION:** Three different combat event enums
```rust
pub enum BossDamageEvent { /* ... */ }
pub enum PartyCombatEvent { /* ... */ }
pub enum PvPCombatEvent { /* ... */ }
```

**✅ CORRECT:** Unified event system
```rust
// crates/mmorpg-combat/src/events.rs
pub enum CombatEvent {
    Damage {
        source_id: Uuid,
        target_id: Uuid,
        amount: u64,
        damage_type: DamageType,
        is_critical: bool,
        context: CombatContext,
    },
    Death {
        victim_id: Uuid,
        killer_id: Option<Uuid>,
        assisted_by: Vec<Uuid>,
        context: CombatContext,
    },
    Heal {
        source_id: Uuid,
        target_id: Uuid,
        amount: u64,
        context: CombatContext,
    },
}

pub enum CombatContext {
    Boss { boss_instance_id: Uuid },
    Party { party_id: Uuid, monster_id: Uuid },
    PvP { instance_id: Uuid, zone_id: Uuid },
}
```

**Rule:** If you find yourself implementing the same concept twice, extract it to a shared crate.

---

## 3. Scalability Requirements

### Horizontal Scaling

**MUST:**
- Design systems to be partitionable by geography, zone, or shard
- No single server should be a bottleneck
- Support cross-zone communication via message passing

**Example:**
```rust
pub struct DistributedBossManager {
    local_bosses: Arc<DashMap<Uuid, BossInstance>>,
    remote_bosses: Arc<DashMap<Uuid, RemoteBossRef>>,
    network_tx: mpsc::Sender<NetworkMessage>,
}

pub enum RemoteBossRef {
    Remote { server_id: Uuid, boss_id: Uuid },
}
```

### Database Sharding Strategy

**MUST:**
- Use partition keys for all large tables
- Design queries to avoid cross-shard joins
- Cache hot data locally before sharding

```sql
-- GOOD: Partitionable by zone_id
CREATE TABLE boss_instances (
    id UUID PRIMARY KEY,
    zone_id UUID NOT NULL,
    template_id UUID NOT NULL,
    current_hp BIGINT NOT NULL,
    -- Zone-based partitioning
);

-- BAD: Requires cross-shard query
CREATE TABLE boss_damage (
    id UUID PRIMARY KEY,
    boss_instance_id UUID NOT NULL,  -- Cannot shard on this
    player_id UUID NOT NULL,
    damage BIGINT NOT NULL,
);
```

### Zone Sharding

**MUST:**
- Design zones to be independent entities
- Support dynamic zone migration between servers
- Minimize cross-zone dependencies

**Architecture:**
```
┌─────────────────┐
│  Zone Manager   │
│  (Orchestrator) │
└────────┬────────┘
         │
    ┌────┴────┬───────┬──────┐
    │         │       │      │
┌───▼───┐ ┌──▼───┐ ┌▼────┐ ┌▼─────┐
│Zone 1 │ │Zone 2│ │Zone3│ │Zone N│
│Server │ │Server│ │Server│ │Server│
└───────┘ └──────┘ └─────┘ └──────┘
```

**Rule:** Every system must have a documented scaling strategy including partition keys, hot data handling, and cross-zone communication.

---

## 4. Atomic Operations

### Lock-Free Data Structures

**❌ VIOLATION:**
```rust
// Current pattern in ALL plans:
boss_instances: Arc<RwLock<HashMap<Uuid, BossInstance>>>,
parties: Arc<RwLock<HashMap<Uuid, PartyInstance>>>,
pvp_instances: Arc<RwLock<HashMap<Uuid, PvPInstance>>>,

// Problems:
// - RwLock causes write starvation
// - HashMap mutations require exclusive lock
// - Hot paths (damage updates) block entire map
```

**✅ CORRECT:**
```rust
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};

// Concurrent hashmap with sharding (16 shards by default)
boss_instances: Arc<DashMap<Uuid, BossInstance>>,
parties: Arc<DashMap<Uuid, PartyInstance>>,
pvp_instances: Arc<DashMap<Uuid, PvPInstance>>,

// For hot path counters:
pub struct BossInstance {
    pub id: Uuid,
    pub current_hp: Arc<AtomicU64>,
    pub max_hp: u64,
    pub damage_table: DashMap<Uuid, Arc<RwLock<BossDamageStats>>>,
    pub engaged_players: DashMap<Uuid, (), ahash::RandomState>,
}

// Lock-free damage recording:
pub fn record_damage(&self, player_id: Uuid, damage: u64) {
    // Atomic HP update (no lock)
    self.current_hp.fetch_sub(damage, Ordering::Relaxed);
    
    // Get or create damage stats (minimal lock time)
    let stats = self.damage_table
        .entry(player_id)
        .or_insert_with(|| Arc::new(RwLock::new(BossDamageStats::default())));
    
    // Lock only the specific player's stats
    if let Some(mut stats) = stats.try_write() {
        stats.total_damage += damage;
    }
}
```

### Atomic Type Guidelines

**USE ATOMIC FOR:**
- Counters (HP, MP, EXP, Gold)
- IDs (sequence numbers, last processed tick)
- Flags (status flags, state flags)
- Time (timestamps, durations)

**DON'T USE ATOMIC FOR:**
- Complex structures (use Arc<RwLock<T>> or DashMap)
- Collections (use DashMap or crossbeam channels)
- Non-primitive types

### DashMap Lock Management

**⚠️ CRITICAL: Preventing Deadlocks**

DashMap uses sharded locking (16 shards by default). Each shard has its own RwLock. When you call `get()`, you acquire a read lock on the specific shard containing the key.

**❌ DEADLOCK PATTERN:**
```rust
// NEVER do this - will hang indefinitely!
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?; // Locks ONE shard
    
    // ❌ DEADLOCK: Trying to iterate while holding shard lock
    for item in self.active_loot.iter() {
        // Can't acquire locks on other shards!
        println!("Item: {:?}", item);
    }
    
    // ❌ DEADLOCK: Can't remove either
    self.active_loot.remove(item_id);
    
    Ok(())
}
```

**✅ CORRECT PATTERN:**
```rust
// ALWAYS extract data and drop reference before other operations
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?;
    
    // Extract needed values immediately
    let expires_at = loot.expires_at;
    let pickup_method = loot.pickup_method.clone();
    drop(loot); // Release shard lock before any other DashMap operations
    
    // Now safe to iterate, remove, or perform other operations
    if Instant::now() > expires_at {
        return Err(anyhow!("Loot expired"));
    }
    
    self.active_loot.remove(item_id); // No deadlock!
    
    Ok(())
}
```

**DashMap Golden Rules:**

1. **Minimize Lock Scope:**
   ```rust
   // ✅ Good: Extract and drop
   let value = map.get(&key)?.value.clone();
   drop(map_ref); // Lock released immediately
   
   // ❌ Bad: Hold lock across operations
   let value = map.get(&key)?;
   some_other_operation(&value); // Still holding lock!
   ```

2. **Use `try_get` for Non-Blocking Access:**
   ```rust
   // ✅ Non-blocking check
   if let Some(item) = self.items.try_get(&item_id) {
       // Process item without blocking
       return Ok(item.value.clone());
   }
   ```

3. **Use `entry` for Atomic Updates:**
   ```rust
   // ✅ Atomic get-or-insert and modify
   self.items.entry(key)
       .and_modify(|item| item.count += 1)
       .or_insert_with(|| Item::new(key));
   ```

4. **Never Hold References Across `.await`:**
   ```rust
   // ❌ DEADLOCK waiting for async while holding lock
   async fn process(&self) {
       let item = self.map.get(&key)?;
       async_operation().await; // Still holding lock!
   }
   
   // ✅ Extract before await
   async fn process(&self) {
       let data = self.map.get(&key)?.value.clone();
       drop(map_ref);
       async_operation().await; // Safe now
   }
   ```

5. **Batch Operations When Possible:**
   ```rust
   // ✅ Single lock for multiple operations
   if let Some(mut item) = self.items.get_mut(&key) {
       item.count += 1;
       item.last_updated = Instant::now();
       item.processed = true;
       // All done with one lock
   }
   ```

### Performance Targets

**MUST ACHIEVE:**
- **Damage recording:** < 100ns per hit (no blocking)
- **Position update:** < 50ns per update (no allocation)
- **State query:** < 500ns for local entity
- **Hot path:** No locks, no allocations, no copies

**Benchmark Command:**
```bash
cargo bench --bench damage_tracking
cargo bench --bench position_updates
```

**Rule:** All hot paths (damage, position, state queries) must be lock-free. Use DashMap for concurrent maps, atomic types for counters.

---

## 4.1 Rust Concurrency & Resource Management

**Core Principle:** Never spawn a thread or task without a defined lifespan. Rely on Ownership and Drop to automate cleanup.

### 4.1.1 Prefer Scoped Concurrency

If the background work is related to a specific scope (e.g., a function), do not use `std::thread::spawn`. It detaches the thread, making it easy to "forget." Use `std::thread::scope` instead.

**Why:** Scoped threads automatically join (wait) at the end of the block. You literally cannot leak them.

**❌ VIOLATION (from benchmarks/tests):**
```rust
// Detached thread might outlive the context
for _ in 0..num_tasks {
    let cache_clone = Arc::clone(&cache);
    let handle = std::thread::spawn(move || {
        let entity_id = Uuid::now_v7();
        let _ = cache_clone.get(entity_id);
    });
    handles.push(handle);
}
```

**✅ CORRECT:**
```rust
// Guaranteed to finish before function returns
std::thread::scope(|s| {
    for _ in 0..num_tasks {
        let cache_clone = Arc::clone(&cache);
        s.spawn(move || {
            let entity_id = Uuid::now_v7();
            let _ = cache_clone.get(entity_id);
        });
    }
}); // All threads joined here automatically
```

### 4.1.2 Use RAII for Long-Lived Workers

If a struct owns a background thread, implement Drop to ensure the thread stops when the struct dies.

**Why:** In Rust, you don't need users to remember to call `stop()`. Drop guarantees cleanup logic runs when the variable goes out of scope.

**❌ VIOLATION (from mmorpg-ffi/src/client.rs):**
```rust
pub unsafe extern "C" fn network_connect(/* ... */) {
    // Spawn send bridge thread: std::mpsc → WebTransport
    let _bridge_tx_handle = thread::spawn(move || {
        let rt = Runtime::new().expect("Bridge: Failed to create runtime");
        rt.block_on(async move {
            while let Ok(data) = outbound_rx.recv() {
                client_send.send_datagram(data).await;
            }
        });
    });

    // Spawn receive bridge thread: WebTransport → std::mpsc
    let _bridge_rx_handle = thread::spawn(move || {
        let rt = Runtime::new().expect("Bridge: Failed to create runtime");
        rt.block_on(async move {
            loop {
                match client_recv.receive_datagram().await {
                    Ok(datagram) => {
                        // Process datagram
                    }
                    Err(_) => break,
                }
            }
        });
    });
    
    // ❌ PROBLEM: Handles are dropped with underscore prefix
    // Threads run forever until process termination!
    // No way to join or stop them cleanly.
}
```

**✅ CORRECT:**
```rust
pub struct WebTransportBridge {
    // Store handles to join on drop
    tx_handle: Option<std::thread::JoinHandle<()>>,
    rx_handle: Option<std::thread::JoinHandle<()>>,
    // Use channel to signal shutdown
    shutdown_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl WebTransportBridge {
    pub fn new(client: WebTransportClient, 
               outbound_rx: std::sync::mpsc::Receiver<Vec<u8>>,
               inbound_tx: std::sync::mpsc::Sender<Vec<u8>>) -> Self {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        
        let tx_handle = std::thread::spawn({
            let client = client.clone();
            let shutdown_rx = shutdown_rx.clone();
            move || {
                let rt = Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    loop {
                        tokio::select! {
                            result = outbound_rx.recv() => {
                                match result {
                                    Ok(data) => {
                                        if client.send_datagram(data).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            _ = shutdown_rx.recv() => {
                                // Shutdown signal received
                                break;
                            }
                        }
                    }
                });
            }
        });

        let rx_handle = std::thread::spawn({
            let shutdown_rx = shutdown_rx.clone();
            move || {
                let rt = Runtime::new().expect("Failed to create runtime");
                rt.block_on(async move {
                    loop {
                        tokio::select! {
                            result = client.receive_datagram() => {
                                match result {
                                    Ok(datagram) => {
                                        if inbound_tx.send(datagram.to_vec()).is_err() {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            _ = shutdown_rx.recv() => {
                                break;
                            }
                        }
                    }
                });
            }
        });

        Self {
            tx_handle: Some(tx_handle),
            rx_handle: Some(rx_handle),
            shutdown_tx: Some(shutdown_tx),
        }
    }
}

impl Drop for WebTransportBridge {
    fn drop(&mut self) {
        // 1. Send shutdown signal by dropping sender
        drop(self.shutdown_tx.take());
        
        // 2. Wait for threads to finish (with timeout)
        if let Some(handle) = self.tx_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.rx_handle.take() {
            let _ = handle.join();
        }
    }
}
```

### 4.1.3 Leverage Channel Disconnection

Never create a "stop" flag (like a bool) when you can use the channel itself.

**Why:** In Rust, dropping a Sender automatically sends an error/notification to the Receiver. Loops waiting on that channel will exit naturally.

**❌ VIOLATION:**
```rust
struct BackgroundWorker {
    stopper: Arc<AtomicBool>,
}

impl BackgroundWorker {
    pub fn spawn(&self) {
        let stopper = Arc::clone(&self.stopper);
        std::thread::spawn(move || {
            while !stopper.load(Ordering::Relaxed) {
                // Do work
            }
        });
    }
    
    pub fn stop(&self) {
        self.stopper.store(true, Ordering::Relaxed);
    }
}

// ❌ PROBLEM: User must remember to call stop()
// If struct is dropped without calling stop(), thread runs forever!
```

**✅ CORRECT:**
```rust
struct BackgroundWorker {
    stopper: std::sync::mpsc::Sender<()>, // Dropping this closes the channel
}

impl BackgroundWorker {
    pub fn spawn(&self) -> std::thread::JoinHandle<()> {
        let receiver = self.stopper.clone().1; // Clone the Receiver
        std::thread::spawn(move || {
            while let Ok(job) = receiver.recv() {
                process(job);
            }
            // Loop breaks automatically when Sender is dropped
        })
    }
}

// ✅ Drop automatically stops the worker - no explicit stop() needed!
```

### 4.1.4 Async: Abort on Drop

If you are using `tokio::spawn`, the task is detached and will run forever even if you lose the handle. If a task must not outlive a struct, wrap the JoinHandle in a struct that calls `.abort()` on drop.

**❌ VIOLATION (from mmorpg-auth/src/main.rs):**
```rust
// Spawns task but doesn't store handle
tokio::spawn(async move {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    loop {
        interval.tick().await;
        // Collect metrics
    }
});

// ❌ PROBLEM: Task runs forever, can't be stopped or aborted
```

**✅ CORRECT:**
```rust
struct PeriodicMetrics {
    handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl PeriodicMetrics {
    pub fn start(db_metrics: Arc<DbMetrics>, auth_metrics: Arc<AuthMetrics>) -> Self {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Collect metrics
                    }
                    _ = &mut shutdown_rx => {
                        info!("Shutting down metrics collector");
                        break;
                    }
                }
            }
        });
        
        Self {
            handle: Some(handle),
            shutdown_tx,
        }
    }
}

impl Drop for PeriodicMetrics {
    fn drop(&mut self) {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(());
        
        // Abort task if it doesn't shut down gracefully
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
```

### 4.1.5 Watch Out for Arc Cycles

Thread leaks in Rust often happen because the thread holds an `Arc` back to the owner, preventing the owner's Drop from ever running.

**Fix:** Use `std::sync::Weak` inside the background thread if it needs to reference the parent.

**❌ VIOLATION (hypothetical):**
```rust
struct BossManager {
    boss_instances: Arc<DashMap<Uuid, BossInstance>>,
}

impl BossManager {
    pub fn start_damage_processor(&self) {
        // ❌ BAD: Thread holds Arc<Self>, preventing Self from dropping
        let manager = Arc::new(self.clone());
        std::thread::spawn(move || {
            loop {
                manager.process_damage();
            }
        });
    }
}
```

**✅ CORRECT:**
```rust
use std::sync::Weak;

impl BossManager {
    pub fn start_damage_processor(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        // ✅ GOOD: Thread holds Weak<Self>. If Self drops, upgrade returns None.
        let weak_manager = Arc::downgrade(&self);
        
        tokio::spawn(async move {
            loop {
                if let Some(manager) = weak_manager.upgrade() {
                    manager.process_damage().await;
                } else {
                    info!("BossManager dropped, exiting damage processor");
                    break;
                }
                
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    }
}
```

### 4.1.6 Real-World Example: SimulatedGameServer

**✅ GOOD PATTERN (from mmorpg-test-utils/src/environments/sim_loop.rs):**

```rust
pub struct SimulatedGameServer {
    is_running: Arc<AtomicBool>,
    http_handle: Option<tokio::task::JoinHandle<()>>,
    sim_loop_handle: Option<std::thread::JoinHandle<()>>,
    webtransport_action_tx: Option<mpsc::Sender<(Uuid, PlayerAction)>>,
}

impl Drop for SimulatedGameServer {
    fn drop(&mut self) {
        info!("Dropping SimulatedGameServer...");
        
        // Stop server
        if self.is_running.swap(false, Ordering::SeqCst) {
            info!("Stopping server...");
        }
        
        // Abort tasks
        if let Some(handle) = self.http_handle.take() {
            handle.abort();
        }
        
        if let Some(handle) = self.sim_loop_handle.take() {
            let _ = handle.join();
        }
    }
}
```

**Mandatory Review Checklist for Concurrency:**

- [ ] Every `std::thread::spawn` is joined or has explicit cleanup path
- [ ] Every `tokio::spawn` stores JoinHandle or is wrapped in Drop
- [ ] Background workers implement Drop with shutdown logic
- [ ] Channel disconnection is used instead of stop flags
- [ ] No Arc cycles between threads and owning structs
- [ ] Use `std::sync::Weak` when background thread needs parent reference

---

## 5. Zero-Copy Architecture

### Eliminate Unnecessary Cloning

**❌ VIOLATION:**
```rust
// Current pattern: Clones entire structures
let next_state = state.clone();  // Expensive!
let party = party.clone();      // Copies all members!
let boss = boss_instance.clone(); // 20+ clones per tick!

// Impact:
// - QuestInstance: 8+ clones per objective update
// - PartyInstance: 15+ clones per member join/leave
// - BossInstance: 20+ clones per damage tick
// - PvPInstance: 50+ clones per combat event
```

**✅ CORRECT:**
```rust
use std::borrow::Cow;
use bytes::Bytes;

// Pattern 1: Use references for read operations
pub fn get_party_info(&self, party_id: Uuid) -> Option<Cow<PartyInstance>> {
    self.parties.get(&party_id).map(Cow::Borrowed)
}

// Pattern 2: In-place mutations
pub fn update_party<F>(&self, party_id: Uuid, update: F)
where
    F: FnOnce(&mut PartyInstance)
{
    if let Some(mut party) = self.parties.get_mut(&party_id) {
        update(&mut party);
    }
}

// Pattern 3: Shared ownership for multiple consumers
pub fn broadcast_party_update(&self, party_id: Uuid) -> Arc<PartyInstance> {
    self.parties.get(&party_id)
        .map(|r| r.value().clone())
        .unwrap()
}

// Pattern 4: Zero-copy network data
#[derive(Serialize, Deserialize)]
pub struct PartyUpdateMessage {
    pub party_id: Uuid,
    pub data: Bytes,  // Zero-copy through network stack
}

pub fn serialize_party_update(party: &PartyInstance) -> Result<Bytes> {
    let mut buffer = BytesMut::new();
    bincode::serialize_into(&mut buffer, party)?;
    Ok(buffer.freeze())
}
```

### Network Serialization Zero-Copy

**❌ VIOLATION:**
```rust
// Multiple allocations and copies
pub fn send_boss_spawn(boss: &BossInstance) {
    let data = bincode::serialize(boss).unwrap();  // Allocation 1
    let compressed = compress(&data).unwrap();    // Allocation 2
    let packet = Packet::new(compressed);         // Allocation 3
    network.send(packet);                          // Copy to socket
}
```

**✅ CORRECT:**
```rust
use bytes::{Bytes, BytesMut};
use std::io::Write;

pub fn send_boss_spawn(boss: &BossInstance) {
    let mut buffer = BytesMut::with_capacity(1024);
    
    // Write directly to buffer (no intermediate allocations)
    boss.write_to(&mut buffer).unwrap();
    
    // Buffer becomes zero-copy network packet
    let packet = Packet::new(buffer.freeze());
    network.send(packet);
}

impl BossInstance {
    pub fn write_to(&self, writer: &mut impl Write) -> std::io::Result<()> {
        writer.write_all(&self.id.as_bytes()[..])?;
        writer.write_u64::<LittleEndian>(self.current_hp)?;
        writer.write_u64::<LittleEndian>(self.max_hp)?;
        // ... more fields
        Ok(())
    }
}
```

### Message Batching

**MUST IMPLEMENT:**
```rust
// Batch multiple updates into single zero-copy message
pub struct StateUpdateBatch {
    updates: Vec<Bytes>,  // Zero-copy updates
    pub timestamp: Instant,
}

impl StateUpdateBatch {
    pub fn new() -> Self {
        Self {
            updates: Vec::with_capacity(100),
            timestamp: Instant::now(),
        }
    }
    
    pub fn add_update(&mut self, update: Bytes) {
        self.updates.push(update);
    }
    
    pub fn into_message(self) -> Bytes {
        let mut buffer = BytesMut::new();
        buffer.put_u32(self.updates.len() as u32);
        for update in self.updates {
            buffer.put_u32(update.len() as u32);
            buffer.put(update);
        }
        buffer.freeze()
    }
}

// Usage:
let mut batch = StateUpdateBatch::new();
for (player_id, pos) in position_updates {
    let update = PositionUpdate::new(player_id, pos);
    batch.add_update(update.serialize_zero_copy()?);
}
network.send(batch.into_message());
```

**Rule:** Minimize allocations and copies. Use references for reads, in-place mutations for writes, shared ownership for sharing, and zero-copy serialization for network.

---

## 6. Performance Standards

### Batching Requirements

**MUST IMPLEMENT:**
```rust
// Batch damage updates per tick
pub struct DamageBatch {
    updates: Vec<DamageUpdate>,
    tick: u64,
}

impl DamageBatch {
    const BATCH_SIZE: usize = 100;
    const FLUSH_INTERVAL_MS: u64 = 50;
    
    pub fn add_update(&mut self, update: DamageUpdate) {
        self.updates.push(update);
        if self.updates.len() >= Self::BATCH_SIZE {
            self.flush();
        }
    }
    
    pub fn flush(&mut self) {
        if self.updates.is_empty() { return; }
        
        // Process all updates in batch
        let updates = std::mem::take(&mut self.updates);
        self.process_batch(updates);
    }
}
```

**Apply batching to:**
- Damage updates (every tick)
- Position updates (every tick)
- Quest objective updates (batch per minute)
- Party member status (batch per second)
- Combat events (batch per tick)
- Database writes (batch per second)

### Caching Strategy

**MUST IMPLEMENT:**
```rust
use lru::LruCache;

pub struct TemplateCache<T> {
    cache: Arc<RwLock<LruCache<Uuid, Arc<T>>>>,
    ttl: Duration,
}

impl<T: Clone + Send + Sync + 'static> TemplateCache<T> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(capacity))),
            ttl,
        }
    }
    
    pub async fn get_or_load<F, Fut>(
        &self,
        id: Uuid,
        loader: F,
    ) -> Result<Arc<T>, Error>
    where
        F: Fn(Uuid) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        // Try cache first
        {
            let cache = self.cache.read().await;
            if let Some(item) = cache.get(&id) {
                return Ok(item.clone());
            }
        }
        
        // Load from source
        let item = loader(id).await?;
        let item = Arc::new(item);
        
        // Store in cache
        let mut cache = self.cache.write().await;
        cache.put(id, item.clone());
        
        Ok(item)
    }
}
```

**Cache hot data:**
- Quest templates (read-only)
- Boss templates (read-only)
- Party member lists (frequent reads)
- PvP rankings (frequent reads)
- Drop tables (read-only)

### Performance Benchmarks

**MUST IMPLEMENT BENCHMARKS FOR:**
```rust
// benches/boss_damage_tracking.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_damage_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage_tracking");
    
    for player_count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(player_count), player_count, |b, &count| {
            let boss = BossInstance::new();
            let players: Vec<_> = (0..count).map(|_| Uuid::new_v4()).collect();
            
            b.iter(|| {
                for &player_id in &players {
                    boss.record_damage(player_id, black_box(100));
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_damage_tracking);
criterion_main!(benches);
```

**Performance Targets:**
- Damage tracking: < 100ns per hit (1000 players)
- Position update: < 50ns per update
- Quest progress: < 1μs per update
- Party member join: < 10ms (including DB)
- Boss spawn: < 100ms (including AI initialization)

**Rule:** All hot paths must be benchmarked. Target: < 100ns for lock-free operations, < 1ms for batched operations, < 100ms for DB operations.

---

## 7. Simring (mmorpg-sync) Compatibility

### Integration Requirements

**MUST:** Integrate with `mmorpg-sync` prediction system for all game state changes.

**Existing Sync Types:**
```rust
// crates/mmorpg-sync/src/types.rs
pub struct SyncGameState {
    pub tick: Tick,
    pub last_sequence: u64,
    pub acks: HashMap<Uuid, SequenceId>,
    pub characters: HashMap<Uuid, SyncCharacterState>,
    pub inventory: HashMap<Uuid, SyncItemState>,
}

pub struct SyncCharacterState {
    pub id: Uuid,
    pub pos: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub state_flags: u32,  // Use for status (stunned, pvp_flagged, etc.)
    pub inventory: HashMap<Uuid, u32>,
}

pub enum InputType {
    Move { x: i32, y: i32 },
    CastSpell { target_id: Uuid, spell_id: u32 },
    UseItem { item_id: Uuid },
    Consume { item_id: Uuid, quantity: u32 },
    Equip { item_id: Uuid, slot: String },
    Unequip { slot: String },
    MoveItem { /* ... */ },
}
```

### Quest System Integration

**❌ VIOLATION:**
```rust
// Quest progress not synchronized
pub struct QuestInstance {
    pub id: Uuid,
    pub objective_progress: HashMap<Uuid, u32>,  // Not in SyncGameState!
    pub status: QuestStatus,
}
```

**✅ CORRECT:**
```rust
// Extend sync types for quest prediction
pub struct SyncGameState {
    // ... existing fields ...
    
    // Add quest tracking
    pub active_quests: HashMap<Uuid, Vec<QuestProgress>>,
}

pub struct QuestProgress {
    pub quest_id: Uuid,
    pub objective_id: Uuid,
    pub progress: u32,
    pub completed: bool,
}

// Add quest input types
pub enum InputType {
    // ... existing inputs ...
    
    AcceptQuest { quest_id: Uuid },
    AbandonQuest { quest_id: Uuid },
    CompleteQuest { quest_id: Uuid },
}
```

### Boss System Integration

**❌ VIOLATION:**
```rust
// Boss phases not synchronized
pub struct BossInstance {
    pub current_phase: u32,  // Not in SyncGameState!
    pub enrage_active: bool,
}
```

**✅ CORRECT:**
```rust
// Extend character state for boss phases
pub struct SyncCharacterState {
    // ... existing fields ...
    
    // Add boss-specific state
    pub boss_phase: Option<u32>,
    pub enrage_active: bool,
    pub ability_cooldowns: HashMap<Uuid, u32>,  // ability_id -> ticks
}

// Boss inputs for prediction
pub enum InputType {
    // ... existing inputs ...
    
    BossAbilityCast { ability_id: Uuid, target_id: Uuid },
    BossPhaseTransition { new_phase: u32 },
}
```

### Party System Integration

**❌ VIOLATION:**
```rust
// Party state not synchronized
pub struct PartyInstance {
    pub id: Uuid,
    pub members: HashMap<Uuid, PartyMember>,  // Not in SyncGameState!
}
```

**✅ CORRECT:**
```rust
// Add party tracking to sync state
pub struct SyncGameState {
    // ... existing fields ...
    
    // Add party state
    pub parties: HashMap<Uuid, PartySyncState>,
}

pub struct PartySyncState {
    pub party_id: Uuid,
    pub leader_id: Uuid,
    pub members: Vec<Uuid>,
    pub status: PartyStatus,
}

// Party inputs for prediction
pub enum InputType {
    // ... existing inputs ...
    
    PartyInvite { target_id: Uuid },
    PartyAccept { party_id: Uuid },
    PartyLeave { party_id: Uuid },
    PartyKick { party_id: Uuid, target_id: Uuid },
}
```

### PvP System Integration

**❌ VIOLATION:**
```rust
// PvP flag not synchronized
pub struct PvPParticipant {
    pub is_flagged: bool,  // Should be in SyncCharacterState.state_flags!
}
```

**✅ CORRECT:**
```rust
// Use existing state_flags for PvP flag
impl SyncCharacterState {
    pub const PVP_FLAGGED: u32 = 1 << 0;
    pub const PVP_COMBAT: u32 = 1 << 1;
    pub const STUNNED: u32 = 1 << 2;
    
    pub fn is_pvp_flagged(&self) -> bool {
        self.state_flags & Self::PVP_FLAGGED != 0
    }
    
    pub fn set_pvp_flagged(&mut self, flagged: bool) {
        if flagged {
            self.state_flags |= Self::PVP_FLAGGED;
        } else {
            self.state_flags &= !Self::PVP_FLAGGED;
        }
    }
}

// PvP inputs for prediction
pub enum InputType {
    // ... existing inputs ...
    
    PvPFlag { flag: bool },
    PvPToggleFlag { duration_seconds: u32 },
}
```

### Prediction Rules

**MUST IMPLEMENT:**
```rust
// Quest objective validation for prediction
impl QuestManager {
    pub fn can_complete_objective(
        &self,
        state: &SyncGameState,
        player_id: Uuid,
        objective: &QuestObjective,
    ) -> bool {
        match objective {
            QuestObjective::KillMonster { monster_id, required_count, .. } => {
                // Check if player has killed enough of this monster
                // This must be derived from combat events, not separate counter
                self.get_monster_kill_count(state, player_id, monster_id) >= *required_count
            }
            // ... other objectives
        }
    }
}
```

**Rule:** All game state changes must be part of SyncGameState and InputType for client-side prediction. Use existing state_flags for status conditions.

---

## 8. WebTransport Compatibility

### Protocol Alignment

**MUST:** Align with `mmorpg-webtransport`'s message structure.

**Existing Protocol:**
```rust
// crates/mmorpg-webtransport/src/ffi.rs
pub enum PacketType {
    GameState,      // Server -> Client
    PlayerAction,   // Client -> Server
}

pub struct PacketHeader {
    pub packet_type: PacketType,
    pub sequence: u32,
}

pub struct PlayerPos {
    pub player_id: Uuid,
    pub x: i32,
    pub y: i32,
    pub tick: u64,
}
```

### Message Format Guidelines

**❌ VIOLATION:**
```rust
// Too many message variants, no batching
pub enum PartyServerMessage {
    PartyCreated,
    PartyJoined,
    PartyLeft,
    PartyDisbanded,
    MemberInvited,
    InviteReceived,
    // ... 40+ more variants!
}
```

**✅ CORRECT:**
```rust
// Consolidate into unified messages
pub enum ServerMessage {
    // Unified state update (handles all entity changes)
    StateUpdate {
        tick: u64,
        updates: Vec<StateUpdate>,
    },
    
    // Action acknowledgment
    ActionAck {
        sequence: u64,
        result: ActionResult,
    },
}

pub enum StateUpdate {
    Party(PartyUpdate),
    Quest(QuestUpdate),
    Boss(BossUpdate),
    PvP(PvPUpdate),
    Character(CharacterUpdate),
}

pub enum ClientMessage {
    // All actions go through PlayerAction
    PlayerAction(SyncPlayerAction),
    
    // Queries (rare)
    Query(QueryType),
}
```

### Delta Updates

**MUST IMPLEMENT:**
```rust
pub struct StateDelta {
    pub tick: u64,
    pub base_tick: u64,  // Tick to apply delta to
    pub changes: Vec<StateChange>,
}

pub enum StateChange {
    Character {
        id: Uuid,
        position: Option<(i32, i32)>,      // Some if changed
        hp: Option<i32>,                   // Some if changed
        state_flags: Option<u32>,           // Some if changed
    },
    Party {
        id: Uuid,
        members: Option<Vec<Uuid>>,         // Some if changed
        leader: Option<Uuid>,              // Some if changed
    },
    Quest {
        id: Uuid,
        progress: Option<HashMap<Uuid, u32>>, // objective_id -> progress
    },
}

// Serialize delta (minimal data)
pub fn serialize_delta(delta: &StateDelta) -> Result<Bytes> {
    let mut buffer = BytesMut::new();
    
    // Only serialize changed fields
    buffer.put_u64(delta.tick);
    buffer.put_u64(delta.base_tick);
    buffer.put_u32(delta.changes.len() as u32);
    
    for change in &delta.changes {
        match change {
            StateChange::Character { id, position, hp, state_flags } => {
                buffer.put_u8(0x01); // Character type
                buffer.put_all(&id.as_bytes()[..]);
                
                // Flag-based field encoding
                let mut flags: u8 = 0;
                if position.is_some() { flags |= 0x01; }
                if hp.is_some() { flags |= 0x02; }
                if state_flags.is_some() { flags |= 0x04; }
                buffer.put_u8(flags);
                
                // Only serialize changed fields
                if let Some(pos) = position {
                    buffer.put_i32(pos.0);
                    buffer.put_i32(pos.1);
                }
                if let Some(hp_val) = hp {
                    buffer.put_i32(*hp_val);
                }
                if let Some(flags_val) = state_flags {
                    buffer.put_u32(*flags_val);
                }
            }
            // ... other change types
        }
    }
    
    Ok(buffer.freeze())
}
```

### Compression Strategy

**MUST IMPLEMENT:**
```rust
use zstd::stream::Encoder;

pub struct CompressedState {
    pub uncompressed_size: u32,
    pub data: Bytes,
}

pub fn compress_state(data: &[u8]) -> Result<CompressedState> {
    let mut encoder = Encoder::new(Vec::new(), 3)?; // Compression level 3 (fast)
    encoder.write_all(data)?;
    let compressed = encoder.finish()?;
    
    Ok(CompressedState {
        uncompressed_size: data.len() as u32,
        data: Bytes::from(compressed),
    })
}
```

### Message Batching

**MUST IMPLEMENT:**
```rust
pub struct StateBatch {
    pub tick: u64,
    pub messages: Vec<(Uuid, ServerMessage)>, // target_id, message
    pub size_hint: usize,
}

impl StateBatch {
    const MAX_SIZE: usize = 1400; // MTU - overhead
    const MAX_MESSAGES: usize = 100;
    
    pub fn add(&mut self, target: Uuid, message: ServerMessage) -> bool {
        let message_size = self.estimate_size(&message);
        
        if self.messages.len() >= Self::MAX_MESSAGES {
            self.flush();
        }
        
        if self.size_hint + message_size > Self::MAX_SIZE {
            self.flush();
        }
        
        self.messages.push((target, message));
        self.size_hint += message_size;
        true
    }
    
    pub fn flush(&mut self) -> Option<NetworkPacket> {
        if self.messages.is_empty() {
            return None;
        }
        
        let messages = std::mem::take(&mut self.messages);
        let packet = NetworkPacket::batch(self.tick, messages);
        
        self.size_hint = 0;
        Some(packet)
    }
}
```

**Rule:** Consolidate message types, use delta updates, compress state data, batch messages to MTU size.

---

## 9. Code Style Guidelines

### Naming Conventions

```rust
// Types: PascalCase
struct QuestInstance;
enum QuestStatus;
trait QuestLoader;

// Functions & Variables: snake_case
fn get_quest_instance(quest_id: Uuid) { }
let quest_id = Uuid::new_v4();

// Constants: SCREAMING_SNAKE_CASE
const MAX_PARTY_SIZE: u32 = 8;
const DEFAULT_TICK_RATE_MS: u32 = 50;

// Type Aliases: PascalCase
type PartyId = Uuid;
type QuestId = Uuid;
```

### Error Handling

**❌ VIOLATION:**
```rust
// Silent failure
fn join_party(&self, party_id: Uuid, player_id: Uuid) {
    let party = self.parties.get(&party_id);
    // What if party doesn't exist? What if full?
}

// Panic in production code
fn create_party(&self, name: String) -> PartyInstance {
    let id = self.generate_id().unwrap(); // Panics on error!
    PartyInstance::new(id, name)
}
```

**✅ CORRECT:**
```rust
// Use Result for fallible operations
pub fn join_party(
    &self,
    party_id: PartyId,
    player_id: PlayerId,
) -> Result<(), PartyError> {
    let mut party = self.parties.get_mut(&party_id)
        .ok_or(PartyError::NotFound { party_id })?;
    
    if party.members.len() >= MAX_PARTY_SIZE {
        return Err(PartyError::PartyFull { party_id, max: MAX_PARTY_SIZE });
    }
    
    party.add_member(player_id);
    Ok(())
}

// Use Option for optional values
pub fn get_party_leader(&self, party_id: PartyId) -> Option<PlayerId> {
    self.parties.get(&party_id)
        .map(|p| p.leader_id)
}

// Never panic in production code
pub fn create_party(&self, name: String) -> Result<PartyId, PartyError> {
    let sanitized_name = validate_party_name(&name)
        .map_err(|e| PartyError::InvalidName { reason: e.to_string() })?;
    
    let id = PartyId::now_v7();
    let party = PartyInstance::new(id, sanitized_name)?;
    
    self.parties.insert(id, party);
    Ok(id)
}
```

### Logging Guidelines

```rust
// Use tracing for structured logging
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self))]
pub fn process_damage(&self, event: DamageEvent) -> Result<()> {
    debug!(
        player_id = %event.player_id,
        boss_id = %event.boss_id,
        damage = event.damage,
        "Processing damage event"
    );
    
    if event.damage == 0 {
        warn!(
            player_id = %event.player_id,
            "Zero damage event received"
        );
        return Ok(());
    }
    
    if event.damage > MAX_DAMAGE_PER_HIT {
        error!(
            player_id = %event.player_id,
            damage = event.damage,
            max_allowed = MAX_DAMAGE_PER_HIT,
            "Damage exceeds maximum allowed"
        );
        return Err(CombatError::InvalidDamage);
    }
    
    // ... process damage
    info!(
        boss_hp = %boss.current_hp,
        "Damage processed successfully"
    );
    
    Ok(())
}
```

**Log Levels:**
- `error`: Unrecoverable errors, security violations
- `warn`: Recoverable errors, suspicious behavior
- `info`: Significant state changes, business events
- `debug`: Detailed flow information for troubleshooting
- `trace`: Very detailed information (performance critical only)

### Documentation Standards

```rust
/// Manages boss instances and their lifecycle.
///
/// The BossManager is responsible for:
/// - Spawning boss instances from templates
/// - Tracking damage dealt by players
/// - Managing boss phases and transitions
/// - Handling boss defeat and rewards
///
/// # Architecture
///
/// ```
/// ┌─────────────┐
/// │  Templates  │ ──► Spawn ──► ┌──────────────┐
/// │  (Immutable)│                │   Boss       │
/// └─────────────┘                │   Instance   │
///         ▲                       │  (Mutable)   │
///         │                       └──────┬───────┘
///    Load/Reload                      │
///                                    ▼
///                          ┌──────────────────┐
///                          │  Damage Tables   │
///                          │  (DashMap)       │
///                          └──────────────────┘
/// ```
///
/// # Thread Safety
///
/// All public methods are thread-safe and use lock-free operations where possible.
/// Hot paths (damage recording) are lock-free using `DashMap` and `AtomicU64`.
///
/// # Performance
///
/// - Damage recording: < 100ns per hit
/// - Boss spawn: < 100ms (including AI init)
/// - Phase transition: < 10ms
///
/// # Examples
///
/// ```
/// use mmorpg_boss::{BossManager, BossTemplate};
///
/// # async fn example() -> anyhow::Result<()> {
/// let manager = BossManager::new(config).await?;
///
/// // Spawn a boss
/// let boss_id = manager.spawn_boss(template_id, location).await?;
///
/// // Record damage (lock-free)
/// manager.record_damage(boss_id, player_id, 1000)?;
///
/// // Check boss status
/// let boss = manager.get_boss(boss_id)?;
/// println!("Boss HP: {}/{}", boss.current_hp, boss.max_hp);
/// # Ok(())
/// # }
/// ```
pub struct BossManager<R: BossRepository> {
    templates: Arc<DashMap<Uuid, BossTemplate>>,
    instances: Arc<DashMap<Uuid, BossInstance>>,
    repository: Arc<R>,
    spatial_index: Arc<SpatialIndex>,
    config: BossConfig,
}

impl<R: BossRepository> BossManager<R> {
    /// Creates a new BossManager.
    ///
    /// # Arguments
    ///
    /// * `repository` - Database repository for persistence
    /// * `config` - Configuration for boss behavior
    ///
    /// # Errors
    ///
    /// Returns an error if the repository connection fails.
    pub async fn new(repository: Arc<R>, config: BossConfig) -> Result<Self> {
        Ok(Self {
            templates: Arc::new(DashMap::new()),
            instances: Arc::new(DashMap::new()),
            repository,
            spatial_index: Arc::new(SpatialIndex::new(config.cell_size)),
            config,
        })
    }

    /// Records damage dealt to a boss (lock-free).
    ///
    /// This operation is thread-safe and uses atomic operations,
    /// making it safe to call from any thread without blocking.
    ///
    /// # Performance
    ///
    /// Time complexity: O(1) amortized
    /// Lock-free: Yes
    ///
    /// # Arguments
    ///
    /// * `boss_id` - ID of the boss instance
    /// * `player_id` - ID of the player dealing damage
    /// * `damage` - Amount of damage to record
    ///
    /// # Errors
    ///
    /// Returns `BossError::NotFound` if the boss instance doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// # use mmorpg_boss::BossManager;
    /// # async fn example(manager: &BossManager<()>) -> Result<(), Box<dyn std::error::Error>> {
    /// manager.record_damage(boss_id, player_id, 1000)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn record_damage(
        &self,
        boss_id: Uuid,
        player_id: Uuid,
        damage: u64,
    ) -> Result<()> {
        let boss = self.instances.get(&boss_id)
            .ok_or(BossError::NotFound { boss_id })?;
        
        // Lock-free HP update
        let current_hp = boss.current_hp.fetch_sub(damage, Ordering::Relaxed);
        
        if current_hp <= damage {
            // Boss defeated - transition to next phase
            self.handle_boss_defeat(boss_id, player_id)?;
        }
        
        // Record damage for rewards (lock-free)
        boss.damage_table.entry(player_id)
            .or_insert_with(|| Arc::new(RwLock::new(BossDamageStats::default())));
        
        Ok(())
    }
}
```

---

## 10. Testing Requirements

### Unit Tests

**MUST IMPLEMENT:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_damage_tracking_single_player() {
        let boss = BossInstance::new_test();
        let player_id = Uuid::new_v4();
        
        // Record damage
        boss.record_damage(player_id, 100).unwrap();
        
        // Verify damage recorded
        let stats = boss.get_damage_stats(player_id).unwrap();
        assert_eq!(stats.total_damage, 100);
        assert_eq!(boss.current_hp.load(Ordering::Relaxed), boss.max_hp - 100);
    }

    #[test]
    fn test_damage_tracking_concurrent() {
        use std::thread;
        
        let boss = Arc::new(BossInstance::new_test());
        let player_id = Uuid::new_v4();
        
        // Spawn multiple threads recording damage concurrently
        let handles: Vec<_> = (0..100)
            .map(|_| {
                let boss = Arc::clone(&boss);
                thread::spawn(move || {
                    boss.record_damage(player_id, 10).unwrap();
                })
            })
            .collect();
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify all damage recorded correctly
        let stats = boss.get_damage_stats(player_id).unwrap();
        assert_eq!(stats.total_damage, 1000);
    }

    #[test]
    fn test_party_full() {
        let party = PartyInstance::new_test();
        
        // Fill party to max capacity
        for _ in 0..MAX_PARTY_SIZE {
            let player_id = Uuid::new_v4();
            party.add_member(player_id).unwrap();
        }
        
        // Try to add one more member
        let result = party.add_member(Uuid::new_v4());
        assert!(matches!(result, Err(PartyError::PartyFull { .. })));
    }
}
```

### Integration Tests

**MUST IMPLEMENT:**
```rust
// tests/boss_integration_test.rs
use mmorpg_boss::BossManager;
use mmorpg_combat::DamageTracker;

#[tokio::test]
async fn test_boss_lifecycle() {
    // Setup
    let repository = MockRepository::new();
    let manager = BossManager::new(repository, config).await.unwrap();
    
    // Spawn boss
    let boss_id = manager.spawn_boss(template_id, location).await.unwrap();
    
    // Simulate combat
    let player_id = Uuid::new_v4();
    for _ in 0..10 {
        manager.record_damage(boss_id, player_id, 1000).unwrap();
    }
    
    // Verify boss defeated
    let boss = manager.get_boss(boss_id).unwrap();
    assert_eq!(boss.status, BossStatus::Defeated);
    
    // Verify rewards distributed
    let rewards = repository.get_rewards(boss_id).await.unwrap();
    assert_eq!(rewards.len(), 1);
}

#[tokio::test]
async fn test_party_boss_combat() {
    // Setup
    let party = create_test_party();
    let boss = spawn_test_boss();
    
    // Multiple party members deal damage
    let damage_events = vec![
        (party.members[0], 1000),
        (party.members[1], 1500),
        (party.members[2], 800),
    ];
    
    for (player_id, damage) in damage_events {
        boss.record_damage(player_id, damage).unwrap();
    }
    
    // Verify damage tracking
    let total_damage: u64 = boss.damage_table.iter()
        .map(|entry| entry.value().total_damage.load(Ordering::Relaxed))
        .sum();
    
    assert_eq!(total_damage, 3300);
    
    // Verify rewards distributed by damage contribution
    let rewards = boss.calculate_rewards();
    assert_eq!(rewards.get(&party.members[1]).unwrap().gold, 1500); // Highest damage
}
```

### Performance Tests

**MUST IMPLEMENT:**
```rust
// benches/boss_performance.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use mmorpg_boss::BossManager;

fn bench_damage_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage_tracking");
    
    for player_count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(player_count), player_count, |b, &count| {
            let boss = BossInstance::new_test();
            let players: Vec<_> = (0..count).map(|_| Uuid::new_v4()).collect();
            
            b.iter(|| {
                for &player_id in &players {
                    boss.record_damage(player_id, black_box(100)).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_phase_transition(c: &mut Criterion) {
    let boss = BossInstance::new_test_with_phases();
    
    c.bench_function("phase_transition", |b| {
        b.iter(|| {
            boss.transition_to_next_phase().unwrap();
        });
    });
}

criterion_group!(benches, bench_damage_tracking, bench_phase_transition);
criterion_main!(benches);
```

### Coverage Requirements

**MUST ACHIEVE:**
- Unit test coverage: > 80%
- Integration test coverage: > 60%
- Critical paths (damage, position, rewards): 100%

**Check coverage:**
```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# View coverage
open coverage/index.html
```

---

## 11. Security Considerations

### Input Validation

**MUST VALIDATE:**
```rust
pub fn validate_position(pos: (i32, i32)) -> Result<(), GameError> {
    const WORLD_MIN: i32 = -32768;
    const WORLD_MAX: i32 = 32768;
    
    if pos.0 < WORLD_MIN || pos.0 > WORLD_MAX {
        return Err(GameError::InvalidPosition {
            x: pos.0,
            reason: format!("Position X out of bounds [{}, {}]", WORLD_MIN, WORLD_MAX),
        });
    }
    
    if pos.1 < WORLD_MIN || pos.1 > WORLD_MAX {
        return Err(GameError::InvalidPosition {
            y: pos.1,
            reason: format!("Position Y out of bounds [{}, {}]", WORLD_MIN, WORLD_MAX),
        });
    }
    
    Ok(())
}

pub fn validate_damage(damage: u64) -> Result<(), GameError> {
    const MAX_DAMAGE_PER_HIT: u64 = 999_999;
    
    if damage > MAX_DAMAGE_PER_HIT {
        return Err(GameError::InvalidDamage {
            damage,
            max_allowed: MAX_DAMAGE_PER_HIT,
        });
    }
    
    if damage == 0 {
        warn!("Zero damage event received");
        return Err(GameError::InvalidDamage {
            damage,
            max_allowed: MAX_DAMAGE_PER_HIT,
        });
    }
    
    Ok(())
}
```

### Rate Limiting

**MUST IMPLEMENT:**
```rust
use std::time::{Duration, Instant};
use std::collections::HashMap;

pub struct RateLimiter {
    buckets: HashMap<Uuid, TokenBucket>,
    window: Duration,
    max_tokens: u32,
}

struct TokenBucket {
    tokens: u32,
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(window: Duration, max_tokens: u32) -> Self {
        Self {
            buckets: HashMap::new(),
            window,
            max_tokens,
        }
    }
    
    pub fn check(&mut self, player_id: Uuid) -> bool {
        let now = Instant::now();
        let bucket = self.buckets.entry(player_id).or_insert(TokenBucket {
            tokens: self.max_tokens,
            last_update: now,
        });
        
        // Refill tokens
        let elapsed = now.duration_since(bucket.last_update);
        let refill = (elapsed.as_secs_f64() / self.window.as_secs_f64()) as u32 * self.max_tokens;
        bucket.tokens = (bucket.tokens + refill).min(self.max_tokens);
        bucket.last_update = now;
        
        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }
}

// Usage for action rate limiting
pub struct ActionRateLimiter {
    damage: RateLimiter,  // 10 hits per second
    chat: RateLimiter,    // 5 messages per second
    party: RateLimiter,   // 1 action per 100ms
}

impl ActionRateLimiter {
    pub fn can_damage(&mut self, player_id: Uuid) -> bool {
        self.damage.check(player_id)
    }
}
```

### Anti-Cheat

**MUST IMPLEMENT:**
```rust
pub struct AntiCheat {
    damage_analyzer: DamageAnalyzer,
    position_analyzer: PositionAnalyzer,
    party_analyzer: PartyAnalyzer,
}

impl AntiCheat {
    pub fn validate_damage(&self, event: &DamageEvent) -> Result<(), CheatError> {
        // Check damage rate
        if self.damage_analyzer.is_damage_suspicious(event) {
            warn!(
                player_id = %event.player_id,
                damage = event.damage,
                rate = self.damage_analyzer.get_damage_rate(event.player_id),
                "Suspicious damage rate detected"
            );
            return Err(CheatError::SuspiciousDamage);
        }
        
        // Check damage range
        let expected_range = self.damage_analyzer.get_expected_damage_range(
            event.player_id,
            event.target_id,
        );
        
        if event.damage < expected_range.0 || event.damage > expected_range.1 {
            warn!(
                player_id = %event.player_id,
                damage = event.damage,
                expected = ?expected_range,
                "Damage outside expected range"
            );
            return Err(CheatError::InvalidDamage);
        }
        
        Ok(())
    }
    
    pub fn validate_position(
        &self,
        player_id: Uuid,
        old_pos: (i32, i32),
        new_pos: (i32, i32),
        dt: Duration,
    ) -> Result<(), CheatError> {
        // Check movement speed
        let distance = calculate_distance(old_pos, new_pos);
        let speed = distance as f64 / dt.as_secs_f64();
        let max_speed = self.position_analyzer.get_max_speed(player_id);
        
        if speed > max_speed {
            warn!(
                player_id = %player_id,
                speed,
                max_speed,
                "Movement speed exceeds maximum"
            );
            return Err(CheatError::SpeedHack);
        }
        
        Ok(())
    }
}
```

---

## 12. Documentation Requirements

### Code Documentation

**EVERY PUBLIC ITEM MUST HAVE:**
- Brief description (one sentence)
- Detailed explanation (when needed)
- Usage examples
- Performance characteristics
- Thread safety guarantees
- Panics (if any)
- Errors (if fallible)

### Architecture Documentation

**EVERY NEW SYSTEM MUST INCLUDE:**
- High-level overview
- Architecture diagram
- Component interaction
- Data flow
- Performance characteristics
- Scalability strategy
- Integration points

### API Documentation

**MUST PROVIDE:**
- Endpoint descriptions
- Request/response formats
- Error codes
- Rate limits
- Authentication requirements

---

## 13. Deployment Requirements

### Configuration Management

**MUST USE:**
```toml
# config/production.toml
[server]
host = "0.0.0.0"
port = 8080
workers = 16

[boss]
max_instances_per_zone = 5
ai_update_interval_ms = 100
damage_batch_size = 100

[party]
max_members = 8
max_level_difference = 20
invite_timeout_seconds = 60

[pvp]
daily_honor_cap = 50000
matchmaking_timeout_seconds = 300

[performance]
tick_rate_hz = 20
state_snapshot_interval_ms = 1000
```

### Monitoring

**MUST IMPLEMENT:**
```rust
use prometheus::{Counter, Histogram, Gauge};

pub struct Metrics {
    // Boss metrics
    boss_damage_total: Counter,
    boss_spawn_duration: Histogram,
    boss_active_count: Gauge,
    
    // Party metrics
    party_join_duration: Histogram,
    party_active_count: Gauge,
    
    // PvP metrics
    pvp_match_duration: Histogram,
    pvp_active_players: Gauge,
}

impl Metrics {
    pub fn record_damage(&self, amount: u64) {
        self.boss_damage_total.inc_by(amount as f64);
    }
    
    pub fn observe_spawn(&self, duration: Duration) {
        self.boss_spawn_duration.observe(duration.as_secs_f64());
    }
}
```

### Logging Configuration

**MUST USE:**
```toml
# logging/production.toml
[filters]
"mmorpg_boss" = "info"
"mmorpg_party" = "info"
"mmorpg_pvp" = "warn"
"tokio" = "warn"

[format]
timestamp = true
level = true
target = true
span_events = true
```

---

## 14. Review Checklist for Plans

Before approving ANY plan, complete this checklist:

### Architecture

- [ ] **SOLID Compliance**: Each component has single responsibility, uses traits for abstraction
- [ ] **DRY Compliance**: No duplicated code, shared types extracted
- [ ] **Scalability**: Horizontal scaling strategy defined, no single points of failure
- [ ] **Simring Compatible**: Integrates with `mmorpg-sync`, extends `SyncGameState` and `InputType`
- [ ] **WebTransport Compatible**: Aligns with protocol, uses delta updates and batching

### Performance

- [ ] **Atomic Operations**: Hot paths use DashMap, atomic types, lock-free data structures
- [ ] **Zero-Copy**: Minimal cloning, uses references and shared ownership
- [ ] **Batching**: Batch operations for hot paths (damage, position, events)
- [ ] **Caching**: Cache strategy defined for hot data
- [ ] **Benchmarks**: Performance benchmarks defined with targets

### Code Quality

- [ ] **Error Handling**: All fallible operations return `Result`, proper error types defined
- [ ] **Logging**: Structured logging with appropriate levels
- [ ] **Documentation**: All public items documented, architecture diagrams provided
- [ ] **Testing**: Unit, integration, and performance tests planned
- [ ] **Coverage**: > 80% unit test coverage, 100% for critical paths

### Security

- [ ] **Input Validation**: All inputs validated before processing
- [ ] **Rate Limiting**: Rate limiting for all user actions
- [ ] **Anti-Cheat**: Anti-cheat validation for critical systems
- [ ] **Authentication**: Proper authentication and authorization

### Deployment

- [ ] **Configuration**: Configuration management defined
- [ ] **Monitoring**: Metrics and monitoring defined
- [ ] **Logging**: Logging configuration defined
- [ ] **Rollback**: Rollback strategy defined

---

## 15. Common Patterns and Anti-Patterns

### DO Use These Patterns

**✅ Concurrent Data Structures:**
```rust
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};

// DashMap for concurrent maps
let map: DashMap<Uuid, BossInstance> = DashMap::new();

// Atomic types for counters
let hp: Arc<AtomicU64> = Arc::new(AtomicU64::new(1000));

// Parking_lot for fast RWLock
let data: Arc<RwLock<Vec<Item>>> = Arc::new(RwLock::new(Vec::new()));
```

**✅ DashMap Lock Management:**
```rust
// ✅ Always extract data and drop the reference before other operations
fn process_item(&self, item_id: &Uuid) -> Result<()> {
    let item = self.items.get(item_id)?;
    
    // Extract needed values immediately
    let value = item.value.clone();
    let expires_at = item.expires_at;
    drop(item); // Release shard lock before any other DashMap operations
    
    // Now safe to iterate, remove, or perform other operations
    if Instant::now() > expires_at {
        self.items.remove(item_id);
    }
    
    Ok(())
}

// ✅ For read-only access, use .try_get() for non-blocking operations
if let Some(item) = self.items.try_get(&item_id) {
    // Item exists and lock is available
}

// ✅ For updates that need multiple fields, use .entry()
self.items.entry(item_id)
    .and_modify(|item| item.count += 1)
    .or_insert_with(|| Item::new(item_id));
```

**✅ Trait Objects for Extensibility:**
```rust
pub trait QuestObjectiveHandler: Send + Sync {
    fn check_completion(&self, state: &SyncGameState, player_id: Uuid) -> bool;
    fn update_progress(&self, state: &mut SyncGameState, player_id: Uuid);
}

pub struct KillMonsterHandler;
impl QuestObjectiveHandler for KillMonsterHandler { /* ... */ }
```

**✅ Message Batching:**
```rust
pub struct StateUpdateBatch {
    updates: Vec<StateUpdate>,
    tick: u64,
    size_hint: usize,
}

impl StateUpdateBatch {
    const MAX_SIZE: usize = 1400; // MTU
    
    pub fn add(&mut self, update: StateUpdate) {
        // Check size and flush if needed
        if self.size_hint + update.estimate_size() > Self::MAX_SIZE {
            self.flush();
        }
        self.updates.push(update);
    }
}
```

### DON'T Use These Patterns

**❌ RwLock for Hot Paths:**
```rust
// BAD: Blocks all reads during writes
boss_instances: Arc<RwLock<HashMap<Uuid, BossInstance>>>,

// GOOD: Concurrent hashmap with sharding
boss_instances: Arc<DashMap<Uuid, BossInstance>>,
```

**❌ Unnecessary Cloning:**
```rust
// BAD: Clones entire structure
let boss = self.boss_instances.get(&id).unwrap().clone();

// GOOD: Reference
let boss = self.boss_instances.get(&id).unwrap();
// Or shared ownership
let boss = self.boss_instances.get(&id).unwrap().value().clone();
```

**❌ Enum Exhaustive Matching for Extensibility:**
```rust
// BAD: Hard to extend
match quest_type {
    QuestType::MainStory => { /* ... */ }
    QuestType::SideQuest => { /* ... */ }
    // Need to add match arm for every new type
}

// GOOD: Trait objects
quest_handler.handle_completion(state, player_id);
```

**❌ Silent Failures:**
```rust
// BAD: Panics or ignores errors
let id = uuid_gen().unwrap(); // Panics!
let result = do_something(); // Ignored!

// GOOD: Proper error handling
let id = uuid_gen()?;
let result = do_something().map_err(|e| {
    error!("Operation failed: {}", e);
    MyError::OperationFailed { source: e }
})?;
```

**❌ Holding DashMap References During Operations:**
```rust
// BAD: Holding reference causes deadlock during iteration
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?; // Holds shard lock
    
    // ❌ This will DEADLOCK - trying to iterate while holding shard lock
    for item in self.active_loot.iter() {
        // Can't acquire locks on other shards!
    }
    
    self.active_loot.remove(item_id); // Also blocked
}

// GOOD: Extract values, drop reference, then operate
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?;
    
    // ✅ Extract needed values and drop lock immediately
    let expires_at = loot.expires_at;
    let pickup_method = loot.pickup_method.clone();
    drop(loot); // Release shard lock before other operations
    
    // Now safe to iterate or remove
    self.active_loot.remove(item_id);
    Ok(())
}
```

**Critical Rule for DashMap:**
- When you call `get()`, you acquire a read lock on ONE shard
- Holding a shard lock while trying to iterate causes deadlock
- Always extract needed data and `drop()` the reference before any other DashMap operations
- Never hold a DashMap reference across async `.await` points or iterations


---

### ⚠️ CRITICAL WARNING: DashMap Deadlock Pattern

**This has caused 2 separate hangs in production code. Memorize this pattern!**

**The Deadlock:**
```rust
// ❌ This will hang forever - tested twice!
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?; // Locks shard #3
    
    // Hangs here - trying to iterate over ALL shards while holding #3
    for item in self.active_loot.iter() {
        // Can't acquire locks on shards 0,1,2,4-15!
    }
    
    self.active_loot.remove(item_id); // Also blocked
}
```

**The Fix (ALWAYS DO THIS):**
```rust
// ✅ This pattern works - extract and drop immediately
fn pickup_loot(&self, item_id: &Uuid, player_id: Uuid) -> Result<()> {
    let loot = self.active_loot.get(item_id)?;
    
    // 1. Extract ALL values you need
    let expires_at = loot.expires_at;
    let pickup_method = loot.pickup_method.clone();
    
    // 2. DROP the reference to release the shard lock
    drop(loot);
    
    // 3. NOW you can iterate, remove, or do other operations
    if Instant::now() > expires_at {
        return Err(anyhow!("Loot expired"));
    }
    self.active_loot.remove(item_id);
    
    Ok(())
}
```

**Why This Happens:**
- DashMap uses 16 shards (by default)
- Each shard has its own RwLock
- `get()` acquires read lock on ONE shard
- Iterating tries to acquire read locks on ALL 16 shards
- Deadlock when holding shard lock while trying to iterate

**Memorize These Rules:**
1. `get()` → extract values → `drop()` → do other operations
2. Never hold DashMap reference across `.await` points
3. Never iterate while holding a DashMap reference
4. Use `try_get()` for non-blocking checks
5. Use `entry()` for atomic updates

**Test This Pattern:**
```bash
# Before fix: test hangs > 60 seconds
cargo test -p mmorpg-server boss::loot::tests::test_pickup_free_for_all

# After fix: test passes in < 1 second
cargo test -p mmorpg-server boss::loot::tests -- --nocapture
```

---

## Conclusion

These principles are non-negotiable requirements for MMORPG development. Every plan must be reviewed against these standards before implementation begins.

**Key Takeaways:**

1. **Don't Trust, Verify** - Challenge every assumption
2. **Atomic Operations** - Hot paths must be lock-free
3. **Zero-Copy** - Minimize allocations and clones
4. **SOLID & DRY** - Clean, maintainable architecture
5. **Performance First** - Benchmark everything
6. **Simring Integration** - All state must be predictable
7. **WebTransport Alignment** - Unified protocol
8. **DashMap Deadlock Prevention** - Extract and drop references before operations

When in doubt, refer to the existing implementation in:
- `crates/mmorpg-sync/` - State synchronization
- `crates/mmorpg-webtransport/` - Network protocol
- `crates/mmorpg-combat/` - Combat system (shared types)
- `crates/mmorpg-loot/` - Loot distribution (shared types)

---

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [SIMD and CPU Optimizations](https://doc.rust-lang.org/std/simd/)
- [Tokio: Rust's asynchronous runtime](https://tokio.rs/)
- [DashMap: Concurrent HashMap](https://github.com/xacrimon/dashmap)
- [Parking Lot: Fast synchronization primitives](https://github.com/Amanieu/parking_lot)

---

**Version:** 1.0.0  
**Last Updated:** 2025-01-23  
**Maintainer:** Architecture Team
