# Handover: 008 Architecture Update - Cloudflare Workers + Durable Objects + SQLite

## What Happened

Updated Plan 008 (XIGNCODE3 Implementation) to align with Cloudflare's modern architecture featuring SQLite-backed Durable Objects. This resolves critical architectural conflicts from the original plan and aligns with project guidelines.

### Key Changes Made

#### 1. Client Architecture (008a)
**Before:** Cloudflare Workers + WASM
- Problem: WASM cannot access Windows APIs needed for detection
- Problem: `no_std` constraints limited detection capabilities

**After:** Native Windows Rust
- Full Windows API access via `windows-sys`
- No WASM or `no_std` requirements
- Direct hardware access for API hooking, process detection
- Uses `retour` for API hooking
- Uses `blake3` for hash verification

#### 2. Server Architecture (008b)
**Before:** Complex multi-tier caching (Moka L1, Redis L2, PostgreSQL L3)
**After:** Cloudflare Workers + Durable Objects with SQLite

**Key Innovation:**
- SQLite runs in same thread as application code
- Synchronous queries with effectively zero latency (microseconds)
- No `await` needed for typical operations
- Output Gates automatically confirm write durability

**Durable Object Types:**
- **SessionObject**: Player sessions, ban status, heartbeat (one per player)
- **PatternObject**: Security pattern distribution (single global instance)
- **TelemetryObject**: Detection event aggregation (100 shards)
- **RateLimiter**: Token bucket rate limiting (sharded)

#### 3. Docker Containers
**Added:** Axum backend in Docker containers for heavy processing
- ML inference (Random Forest, Neural Networks)
- Complex behavioral analysis
- Long-term storage in PostgreSQL
- Report generation and analytics

#### 4. Technology Decisions
- **Axum**: Confirmed as web framework (not Ntex from research)
  - Justification: Ecosystem maturity > marginal performance gain (18K vs 23K RPS)
  - Aligns with "perf/sec" principle via ergonomics

- **SQLx with raw_sql**: Per project guidelines for PgCat compatibility
- **blake3**: Per project guidelines for all hashing
- **argon2**: Per project guidelines for password hashing
- **Uuid::v7()**: Per project guidelines for ID generation

## Where is the Plan/Code/Test

### Updated Documents

1. **`plans/008_xigncode3_impl/008a_client_foundation.md`**
   - Native Windows client implementation
   - API hooking, process detection, macro detection
   - OS integrity validation, anti-debugging
   - Unity FFI integration
   - Full Windows API access

2. **`plans/008_xigncode3_impl/008b_client_server_comm.md`**
   - Cloudflare Workers (Rust) request routing
   - Durable Objects with SQLite implementation
   - Session, Pattern, Telemetry, Rate Limit DOs
   - Docker container integration with Axum
   - Authentication and rate limiting

3. **`plans/008_xigncode3_impl/README.md`**
   - Updated overview of all phases
   - New architecture summary
   - Performance targets updated
   - Dependencies updated

4. **`plans/008_xigncode3_impl/ARCHITECTURE_SUMMARY.md`** (NEW)
   - Comprehensive architecture documentation
   - Component breakdown with diagrams
   - Data flow diagrams
   - Scaling strategy
   - Security architecture
   - Performance optimization techniques
   - Deployment architecture
   - Cost analysis
   - Risk mitigations

### Project Structure (After Implementation)

```
maxion-protector/
├── crates/
│   ├── maxion-antihack/          # NEW - Native Windows client
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs
│   │       ├── detection/
│   │       │   ├── api.rs         # API hooking
│   │       │   ├── process.rs    # Process injection
│   │       │   ├── macro.rs      # Hardware macro
│   │       │   ├── integrity.rs  # OS validation
│   │       │   └── antidebug.rs # Anti-debugging
│   │       └── server_client.rs  # Server communication
│   │
│   ├── maxion-server-worker/      # NEW - Cloudflare Workers
│   │   ├── Cargo.toml
│   │   ├── wrangler.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── router.rs        # Request routing
│   │   │   ├── auth.rs          # Authentication
│   │   │   └── types.rs
│   │   └── durable_objects/
│   │       ├── session.rs        # Session management
│   │       ├── pattern.rs        # Pattern distribution
│   │       ├── telemetry.rs      # Telemetry aggregation
│   │       └── rate_limit.rs    # Rate limiting
│   │
│   └── maxion-axum-backend/     # NEW - Docker container
│       ├── Cargo.toml
│       ├── Dockerfile
│       └── src/
│           ├── main.rs           # Axum server
│           ├── telemetry.rs      # Telemetry analysis
│           ├── patterns.rs       # Pattern extraction
│           └── bans.rs          # Ban management
│
└── tests/
    └── integration/
        ├── client_server_tests.rs
        ├── durable_object_tests.rs
        └── end_to_end_tests.rs
```

### Configuration Files

1. **`wrangler.toml`** (Cloudflare Workers)
   ```toml
   name = "maxion-protector-worker"
   main = "worker.rs"
   compatibility_date = "2024-09-26"
   
   [[durable_objects.bindings]]
   name = "SESSIONS"
   class_name = "SessionObject"
   
   [[migrations]]
   tag = "v1"
   new_sqlite_classes = ["SessionObject", "PatternObject", "TelemetryObject"]
   ```

2. **`Dockerfile`** (Axum Backend)
   ```dockerfile
   FROM rust:1.75-slim
   WORKDIR /app
   COPY Cargo.toml Cargo.lock ./
   COPY src ./src
   RUN cargo build --release --profile.max-opt
   CMD ["./target/release/maxion-axum-backend"]
   ```

## Reflection

### Struggling

1. **Original WASM vs Windows API Conflict**
   - Original plan: Client in WASM for Cloudflare Workers
   - Problem: WASM cannot access Windows APIs needed for detection
   - Solution: Use native Windows client, Workers only for server-side

2. **Ntex vs Axum Framework Choice**
   - Research 007a recommended Ntex for raw performance (23K RPS)
   - User chose Axum (18K RPS)
   - Solution: Documented justification - ecosystem maturity > marginal performance gain

3. **Database Architecture Complexity**
   - Original plan: Multi-tier caching (Moka, Redis, PostgreSQL)
   - Complexity: Multiple layers to manage
   - Solution: Simplified with Durable Objects + SQLite (zero latency)

### Solved

1. **Zero-Latency State Management**
   - Challenge: Minimize database latency for real-time detection
   - Solution: SQLite in Durable Objects runs in same thread
   - Result: < 1ms query latency (microseconds for cached data)

2. **Scalability Strategy**
   - Challenge: How to scale stateful anti-cheat system
   - Solution: Durable Objects scale out (not up) by creating more instances
   - Result: Horizontal scaling with automatic sharding

3. **Heavy Processing vs Lightweight Operations**
   - Challenge: ML inference too heavy for Workers
   - Solution: Hybrid architecture - Workers for routing, Docker for processing
   - Result: Best of both worlds

### Key Insights

1. **Durable Objects + SQLite is Revolutionary**
   - No network hop to database
   - No async overhead for common queries
   - Automatic durability via Output Gates
   - Point-in-time recovery built-in

2. **Native Client is Mandatory for Anti-Cheat**
   - Full Windows API access required
   - Hardware-level monitoring needed
   - WASM sandboxing is a blocker

3. **Hybrid Architecture Works Well**
   - Workers: Routing, validation, lightweight state
   - Durable Objects: Zero-latency state management
   - Docker: Heavy processing, ML, long-term storage

## Remaining Work

### Phase 1: Client Foundation (008a) - 14 days
- [ ] Create `maxion-antihack` crate
- [ ] Implement API hooking with `retour`
- [ ] Implement process injection detection
- [ ] Implement hardware macro detection (K-S test)
- [ ] Implement OS integrity validation with `blake3`
- [ ] Implement anti-debugging & VM detection
- [ ] Create Unity FFI interface
- [ ] Write unit and integration tests
- [ ] Benchmark performance (< 5ms detection latency)

### Phase 2: Client-Server Communication (008b) - 14 days
- [ ] Create `maxion-server-worker` crate
- [ ] Implement Durable Objects with SQLite
  - [ ] SessionObject (registration, heartbeat, ban checks)
  - [ ] PatternObject (distribution)
  - [ ] TelemetryObject (aggregation, 100 shards)
  - [ ] RateLimiter (token bucket)
- [ ] Implement request routing
- [ ] Implement authentication (JWT + `blake3`)
- [ ] Configure `wrangler.toml`
- [ ] Deploy to Cloudflare
- [ ] Write integration tests

### Phase 3: Server Detection (008c) - 15 days
- [ ] Create `maxion-axum-backend` crate
- [ ] Implement Axum server in Docker
- [ ] Integrate with PostgreSQL (`sqlx::raw_sql` per guidelines)
- [ ] Implement ML inference models
- [ ] Implement complex behavioral analysis
- [ ] Integrate with Durable Objects
- [ ] Write performance benchmarks

### Phase 4-6: Pattern Management, Ban Management, Analytics
- [ ] Extend PatternObject with extraction and validation
- [ ] Integrate ban logic with SessionObject
- [ ] Implement metrics collection
- [ ] Build alerting system
- [ ] Create dashboards

## Issues Reference

### Related Issues

None yet - this is a planning phase handover. Issues will be created during implementation.

### Potential Issues to Track

1. **Durable Object Storage Limits**
   - Track: Storage size per DO
   - Alert: > 80% of 1GB limit (beta)
   - Mitigation: Archive old data to PostgreSQL

2. **Single-Threaded DO Performance**
   - Track: Requests per second per DO
   - Alert: > 1000 requests/second
   - Mitigation: Increase sharding

3. **Cold Start Latency**
   - Track: DO initialization time
   - Alert: > 100ms cold start
   - Mitigation: Keep warm DOs for critical sessions

4. **PostgreSQL Connection Pooling with PgCat**
   - Track: Connection pool health
   - Alert: > 90% pool utilization
   - Mitigation: Increase pool size

## How to Dev/Test

### Development Environment Setup

#### 1. Native Windows Client (008a)
```bash
# Clone and setup
cd maxion-protector
cargo new --lib crates/maxion-antihack

# Add dependencies
cd crates/maxion-antihack
cargo add retour windows-sys blake3 orion anyhow thiserror

# Development
cargo build
cargo test
cargo clippy --fix --allow-dirty
```

**Unity Integration:**
```csharp
// Assets/Scripts/AntiHack/AntiHackManager.cs
[DllImport("maxion_antihack")]
private static extern IntPtr detect_anomalies();
```

#### 2. Cloudflare Workers (008b)
```bash
# Install wrangler
npm install -g wrangler

# Login to Cloudflare
wrangler login

# Create worker project
cd crates/maxion-server-worker
wrangler init

# Build Rust to WASM
cargo build --release --target wasm32-wasi

# Deploy
wrangler deploy
```

**Testing Durable Objects:**
```bash
# Run worker locally
wrangler dev

# Test SQLite queries
curl http://localhost:8787/session/test123
```

#### 3. Axum Backend (Docker)
```bash
# Build Docker image
cd crates/maxion-axum-backend
docker build -t maxion-axum-backend:latest .

# Run container
docker run -p 3000:3000 \
  -e DATABASE_URL=postgres://user:pass@host/db \
  maxion-axum-backend:latest

# Test
curl http://localhost:3000/health
```

### Testing Strategy

#### Unit Tests
```bash
# All crates
cargo test --workspace

# Specific crate
cargo test -p maxion-antihack
cargo test -p maxion-server-worker
cargo test -p maxion-axum-backend

# Specific test
cargo test -p maxion-antihack -- test_api_bypass_detection
```

#### Integration Tests
```bash
# Client-Server communication
cargo test --test client_server_tests

# Durable Object interactions
cargo test --test durable_object_tests

# End-to-end flow
cargo test --test end_to_end_tests
```

#### Performance Benchmarks
```bash
# Client detection latency
cargo test -p maxion-antihack --release -- --nocapture benchmark

# Worker response time
wrangler dev --port 8787
# Run: ab -n 10000 http://localhost:8787/session/test123

# SQLite query latency
# Add logging to DOs and measure query times
```

### CI/CD Pipeline

#### GitHub Actions Workflow
```yaml
name: Test 008 Anti-Cheat

on: [push, pull_request]

jobs:
  test-client:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Test client
        run: cargo test -p maxion-antihack

  test-worker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Test worker
        run: cargo test -p maxion-server-worker

  test-backend:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test
    steps:
      - uses: actions/checkout@v3
      - name: Test backend
        run: cargo test -p maxion-axum-backend
```

### Deployment Steps

#### Deploy Cloudflare Workers
```bash
# Build worker
cargo build --release --target wasm32-wasi

# Deploy to production
wrangler deploy --env production

# Deploy to development
wrangler deploy --env development
```

#### Deploy Docker Containers
```bash
# Build and push image
docker build -t registry.example.com/maxion-axum-backend:latest .
docker push registry.example.com/maxion-axum-backend:latest

# Deploy to Kubernetes
kubectl set image deployment/maxion-axum-backend \
  maxion-axum-backend=registry.example.com/maxion-axum-backend:latest

# Rollback if needed
kubectl rollout undo deployment/maxion-axum-backend
```

### Monitoring and Debugging

#### Cloudflare Dashboard
- Monitor: Request rate, error rate, latency
- Durable Objects: Storage size, query performance
- Logs: View real-time logs from workers

#### Docker Logs
```bash
# View logs
kubectl logs -f deployment/maxion-axum-backend

# Stream logs with tracing
RUST_LOG=debug cargo run
```

#### Common Debugging Commands
```bash
# Check Durable Object state
wrangler durable-objects list

# View Worker logs
wrangler tail

# Test authentication
curl -H "Authorization: Bearer <token>" \
  http://localhost:8787/session/test123

# Test rate limiting
for i in {1..100}; do
  curl http://localhost:8787/telemetry/test123
done
```

## Next Steps

1. **Begin Phase 1 (008a)**: Implement native Windows client foundation
2. **Set up Cloudflare account**: Configure Workers and Durable Objects
3. **Create PostgreSQL instance**: For Docker backend storage
4. **Set up CI/CD**: GitHub Actions for automated testing
5. **Write initial tests**: Unit tests for all three crates
6. **Benchmark baseline**: Measure performance before optimization

## Success Criteria

Phase complete when:
- [ ] Native Windows client detects API bypasses, injection, macros
- [ ] Worker routes requests to Durable Objects with < 10ms latency
- [ ] SQLite queries execute in < 1ms (synchronous)
- [ ] All tests passing (>80% coverage)
- [ ] Performance benchmarks meet targets
- [ ] Documentation complete (README, API docs, deployment guide)

## Contacts

- **Architect**: [To be assigned]
- **Lead Developer**: [To be assigned]
- **Cloudflare Support**: https://developers.cloudflare.com/support/
- **Project Repository**: https://github.com/maxion-game/maxion-protector

---

**Document Status:** ✅ Complete  
**Created:** 2025-01-24  
**Next Review:** After Phase 2 (008b) completion  
**Estimated Implementation Start:** [To be scheduled]