# Implementation Checklist: Lightweight Sidecar Anti-Cheat

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-21 |
| Version | 1.0 |
| Complexity | Intermediate |
| Time to Read | 20 minutes |
| Audience | Developers, QA Engineers, DevOps |

## Phase 1: Core Infrastructure (Week 1-2)

### Cloudflare Worker Setup
- [ ] Initialize Cloudflare Workers project with wrangler
- [ ] Configure `wrangler.toml` with:
  - [ ] Worker name: `maxion-anticheat`
  - [ ] Compatibility date
  - [ ] Environment variables (game whitelist, API keys)
  - [ ] Durable Object bindings
  - [ ] KV namespace bindings
- [ ] Setup local development environment
- [ ] Configure CI/CD for automatic deployment

### Axum Router Implementation
- [ ] Create Axum application in `src/worker.rs`
- [ ] Implement CORS handling (strict origin whitelist)
- [ ] Add error handling middleware
- [ ] Add request logging middleware
- [ ] Implement `/register` POST endpoint (player registration with Ed25519 public key)
- [ ] Implement `/action-token` POST endpoint (one-time action token generation)
- [ ] Implement `/cheat` POST endpoint (signed cheat event with dual verification)
- [ ] Implement `/status/{player_id}` GET endpoint (query current player state)
- [ ] Implement `/query/{player_id_prefix}` GET endpoint (batch player queries)
- [ ] Add health check endpoint `/health`

### Request Validation
- [ ] Implement player_id format validator (BLAKE3 hash of Ed25519 public key)
- [ ] Implement timestamp validator (±5 minutes from current time)
- [ ] Implement game_id whitelist check
- [ ] Implement version format validator (semver)
- [ ] Implement cheat_type enum validator
- [ ] Implement Ed25519 signature validator
- [ ] Implement action token validator (BLAKE3 hash check)
- [ ] Implement nonce uniqueness validator (replay prevention)
- [ ] Add rate limiting (per player_id: 100/min, per IP: 1000/min)

### Durable Object Implementation
- [ ] Create `PlayerStateDO` class in `src/do/player.rs`
- [ ] Define SQLite schema:
  - [ ] `sessions` table (player_id, public_key, hwid, ban_status, risk_score)
  - [ ] `cheat_events` table
  - [ ] `used_nonces` table (for replay prevention)
  - [ ] Indexes for efficient queries
- [ ] Implement `register_player()` method (store Ed25519 public key)
- [ ] Implement `generate_action_token()` method (BLAKE3-based token)
- [ ] Implement `record_cheat()` method with dual verification
- [ ] Implement `get_state()` method (synchronous)
- [ ] Implement `check_ban_status()` method (synchronous)
- [ ] Implement status calculation logic (clean/flagged/watching/banned)
- [ ] Add DO initialization logic
- [ ] Implement DO serialization for persistence
- [ ] Create `NonceTrackerDO` class for global nonce tracking
- [ ] Implement nonce expiration cleanup (every 5 minutes)

### KV Integration
- [ ] Create KV namespace in Cloudflare
- [ ] Implement KV write operations in DO
- [ ] Implement KV read operations in DO
- [ ] Implement key format: `{game_id}|{version}|{player_id}`
- [ ] Implement JSON serialization for values
- [ ] Add TTL configuration (default 90 days)

### Testing - Phase 1
- [ ] Unit tests for request validators
- [ ] Unit tests for DO methods
- [ ] Unit tests for status calculation logic
- [ ] Integration tests for all endpoints
- [ ] Load tests (target: 10,000 req/s)
- [ ] Local deployment testing

### Deployment - Phase 1
- [ ] Deploy Worker to Cloudflare Workers
- [ ] Verify Durable Objects are created
- [ ] Verify KV namespace is accessible
- [ ] Test endpoints from local machine
- [ ] Configure custom domain (anticheat.example.com)
- [ ] Setup monitoring (Cloudflare Analytics)

---

## Phase 2: Client Integration (Week 3)

### Client-Side Integration
- [ ] Create `crates/maxion-anticheat-client` crate
- [ ] Implement HTTP client for `/cheat` endpoint
- [ ] Implement retry logic (exponential backoff)
- [ ] Implement local buffering (offline support)
- [ ] Add serialization/deserialization for `CheatEvent`
- [ ] Add configuration management (URL, timeout, retry count)

### Unity FFI Integration
- [ ] Update `maxion_register_cheat_callback` to call sidecar
- [ ] Add FFI function: `maxion_send_cheat_event(game_id, version, uuid, ...)`
- [ ] Add FFI function: `maxion_get_player_status(uuid)`
- [ ] Update Unity C# bindings
- [ ] Update `AntiCheat.cs` class
- [ ] Add async/await for HTTP calls (don't block main thread)

### Error Handling
- [ ] Implement graceful degradation (offline mode)
- [ ] Add local log of failed attempts
- [ ] Implement exponential backoff for retries
- [ ] Add timeout handling (5s default)
- [ ] Log all errors for debugging

### Testing - Phase 2
- [ ] Update `cheat_callback_demo.rs` to use new client
- [ ] Test with actual game client
- [ ] Test offline scenarios
- [ ] Test network failure scenarios
- [ ] Test high-frequency events (stress test)
- [ ] Verify UUID v7 generation

### Documentation - Phase 2
- [ ] Write integration guide for game developers
- [ ] Document FFI API
- [ ] Add example usage in Unity
- [ ] Document configuration options
- [ ] Document error handling patterns

---

## Phase 3: Query Optimization (Week 4)

### Batch Query Implementation
- [ ] Implement `/query` endpoint with:
  - [ ] Prefix filtering
  - [ ] Status filtering
  - [ ] Timestamp range filtering
  - [ ] Pagination support (limit/offset)
  - [ ] Result caching (5s TTL)
- [ ] Add query validation
- [ ] Implement query optimization (avoid full scans)

### KV Query Optimization
- [ ] Test KV `list()` performance
- [ ] Optimize key prefix strategy
- [ ] Implement query batching
- [ ] Add query result caching in DO
- [ ] Add metrics for query performance

### Client-Side Query Helpers
- [ ] Implement batch check function in client
- [ ] Add query result caching
- [ ] Implement parallel query execution
- [ ] Add query timeout handling

### Testing - Phase 3
- [ ] Performance tests for `/query` endpoint
- [ ] Load tests for batch queries (1000 players at once)
- [ ] Test prefix filtering accuracy
- [ ] Test pagination
- [ ] Benchmark query latency (target: < 50ms for 1000 players)

### Deployment - Phase 3
- [ ] Deploy optimized endpoints
- [ ] Update client library
- [ ] Verify query performance in production
- [ ] Setup alerts for slow queries

---

## Phase 4: External System Integration (Week 5)

### Game Server Integration
- [ ] Create integration guide for game servers
- [ ] Implement example: check player before join
- [ ] Implement example: batch check for matchmaking
- [ ] Implement example: sync ban list periodically
- [ ] Provide Rust client library

### Ban System Integration
- [ ] Create sync script to pull banned players from KV
- [ ] Implement webhook endpoint (optional)
- [ ] Document ban decision logic
- [ ] Provide example implementation

### Analytics Integration
- [ ] Create metrics collection script
- [ ] Implement cheat type aggregation
- [ ] Implement violation rate calculation
- [ ] Document available metrics
- [ ] Provide dashboard templates

### Documentation - Phase 4
- [ ] Write complete integration guide
- [ ] Document all API endpoints
- [ ] Provide code examples in multiple languages
- [ ] Create troubleshooting guide
- [ ] Document common integration patterns

### Testing - Phase 4
- [ ] End-to-end integration tests
- [ ] Test with real game server
- [ ] Test with real ban system
- [ ] Test with real analytics system
- [ ] Security audit (external)

---

## Phase 5: Optional Enhancements (Post-Launch)

### Docker Containers (Optional)
- [ ] Setup Cloudflare Docker Container
- [ ] Implement ML-based cheat detection
- [ ] Implement behavioral analysis
- [ ] Integrate with DO (call from DO if needed)
- [ ] Add heavy computation offloading

### Webhook Notifications
- [ ] Implement webhook endpoint configuration
- [ ] Add webhook delivery queue
- [ ] Implement retry logic
- [ ] Add webhook signature verification
- [ ] Document webhook payload format

### Admin Dashboard
- [ ] Create dashboard UI
- [ ] Implement player search
- [ ] Implement status visualization
- [ ] Add manual status override
- [ ] Add export functionality (CSV/JSON)

### Advanced Analytics
- [ ] Implement anomaly detection
- [ ] Add trend analysis
- [ ] Implement predictive models
- [ ] Add custom report generation
- [ ] Create alerting rules

---

## Acceptance Criteria

### Functional Requirements
- [ ] Cheat events recorded within 10ms (p95)
- [ ] Player state queryable via KV
- [ ] Supports prefix filtering (KV list)
- [ ] Nanosecond timestamp accuracy
- [ ] UUID v7 support
- [ ] Offline support (local buffering)

### Performance Requirements
- [ ] < 10ms end-to-end latency (p95)
- [ ] 50,000 cheat events/sec throughput
- [ ] 100,000 status queries/sec
- [ ] < 1GB memory per Durable Object
- [ ] < 50ms batch query for 1000 players

### Security Requirements
- [ ] TLS 1.3 only
- [ ] Replay attack prevention (timestamp validation)
- [ ] Rate limiting (per UUID and per IP)
- [ ] Input validation (all endpoints)
- [ ] CORS strict origin whitelist

### Reliability Requirements
- [ ] 99.9% uptime (Cloudflare SLA)
- [ ] Zero data loss (DO replication)
- [ ] Graceful degradation (offline mode)
- [ ] Automatic retries with backoff
- [ ] Comprehensive error logging

### Documentation Requirements
- [ ] API documentation (all endpoints)
- [ ] Integration guide for game developers
- [ ] Client library documentation
- [ ] Troubleshooting guide
- [ ] Architecture diagram
- [ ] Code examples in multiple languages

---

## Testing Checklist

### Unit Tests
- [ ] Request validators (>90% coverage)
- [ ] Durable Object methods (>90% coverage)
- [ ] KV operations (>90% coverage)
- [ ] Status calculation logic (100% coverage)
- [ ] Client library (>80% coverage)

### Integration Tests
- [ ] End-to-end cheat detection flow
- [ ] All API endpoints
- [ ] Error scenarios
- [ ] Network failures
- [ ] Offline mode

### Load Tests
- [ ] Single player high-frequency (100 events/sec)
- [ ] 10,000 concurrent players
- [ ] 50,000 cheat events/sec
- [ ] 100,000 status queries/sec
- [ ] 1,000 player batch query

### Security Tests
- [ ] Replay attack prevention
- [ ] Rate limiting effectiveness
- [ ] Input validation bypass attempts
- [ ] SQL injection prevention
- [ ] XSS prevention

### Performance Tests
- [ ] Latency benchmarks (p50, p95, p99)
- [ ] Throughput benchmarks
- [ ] Memory usage profiling
- [ ] CPU usage profiling
- [ ] KV query performance

---

## Deployment Checklist

### Pre-Deployment
- [ ] All tests passing
- [ ] Code review completed
- [ ] Documentation updated
- [ ] Configuration verified
- [ ] Monitoring configured
- [ ] Rollback plan documented

### Deployment
- [ ] Deploy Worker to staging
- [ ] Run smoke tests on staging
- [ ] Verify monitoring metrics
- [ ] Deploy Worker to production
- [ ] Verify production endpoints
- [ ] Check error logs

### Post-Deployment
- [ ] Monitor error rates
- [ ] Monitor latency metrics
- [ ] Monitor throughput
- [ ] Check DO storage usage
- [ ] Verify KV operations
- [ ] Notify stakeholders

---

## Monitoring & Alerting

### Metrics to Track
- [ ] Request rate (by endpoint)
- [ ] Error rate (by type)
- [ ] Latency (p50, p95, p99)
- [ ] Durable Object count
- [ ] Durable Object storage usage
- [ ] KV read/write operations
- [ ] KV storage usage
- [ ] Rate limiting violations

### Alerts to Configure
- [ ] Error rate > 1%
- [ ] Latency p95 > 50ms
- [ ] DO storage > 80% of limit
- [ ] KV write failures > 0.1%
- [ ] Unusual traffic patterns

### Dashboards to Create
- [ ] Request overview
- [ ] Error breakdown
- [ ] Latency heatmap
- [ ] DO usage
- [ ] KV usage
- [ ] Cheat type distribution

---

## Cost Optimization

### Before Launch
- [ ] Estimate traffic patterns
- [ ] Calculate projected costs
- [ ] Set budget alerts
- [ ] Optimize KV write frequency
- [ ] Optimize DO storage usage

### Ongoing
- [ ] Monitor cost trends
- [ ] Review Cloudflare billing
- [ ] Optimize expensive operations
- [ ] Consider caching strategies
- [ ] Review data retention policies

---

## Rollback Plan

### Triggers
- [ ] Error rate > 5% for 5 minutes
- [ ] Latency p95 > 100ms for 10 minutes
- [ ] Data consistency issues
- [ ] Security vulnerabilities

### Rollback Steps
- [ ] Switch DNS to previous version
- [ ] Revert Worker deployment
- [ ] Restore KV data from backup (if needed)
- [ ] Notify stakeholders
- [ ] Investigate root cause
- [ ] Document lessons learned

---

## Post-Launch Tasks

### Week 1
- [ ] Monitor system stability
- [ ] Gather feedback from game developers
- [ ] Fix critical bugs
- [ ] Address performance issues
- [ ] Update documentation

### Week 2-4
- [ ] Implement feature requests
- [ ] Optimize based on real usage
- [ ] Add missing monitoring
- [ ] Conduct security review
- [ ] Plan next enhancements

### Month 2-3
- [ ] Evaluate Docker container needs
- [ ] Consider ML implementation
- [ ] Enhance analytics
- [ ] Improve admin tools
- [ ] Expand integration guides

---

## Notes

### Dependencies
- Cloudflare Workers SDK
- Cloudflare Durable Objects
- Cloudflare KV
- Axum (Rust web framework)
- Tokio (async runtime)
- Serde (serialization)
- Uuid (v7 support)
- Chrono (time handling)

### External Dependencies
- Existing auth server (for game servers)
- Existing ban system (for enforcement)
- Existing analytics (for metrics)

### Known Limitations
- DO storage limit: 1GB per object (beta)
- KV write cost is higher than reads
- Docker containers are optional and add latency
- No built-in authentication (external system)

### Risk Mitigation
- Implement graceful degradation
- Add comprehensive monitoring
- Use established patterns (KV hierarchical keys)
- Keep design simple and focused
- Iterate based on real usage

---

**Last Updated:** 2025-01-21  
**Version:** 1.0  
**Maintainer:** Maxion Protector Team