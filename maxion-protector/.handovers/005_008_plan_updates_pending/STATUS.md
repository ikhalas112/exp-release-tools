# Pending Plan Updates for Dual Verification System

**Status:** Created on 2025-01-27  
**Related Handover:** 005_008_action_token_impl.md  
**Priority:** High - Complete before implementation begins

---

## Overview

This document tracks the remaining updates needed to align all plan documents with the new dual verification system (Ed25519 + BLAKE3 action tokens). Due to API rate limits during the initial handover creation, these updates are tracked here for future completion.

---

## Completed Updates ✅

1. ✅ **ARCHITECTURE_SUMMARY.md** - Updated to Version 3.0
   - Added dual verification diagrams (Ed25519 + BLAKE3)
   - Updated security layers section
   - Added replay attack prevention flow
   - Updated data models with player_id, signature, action_token

2. ✅ **008c_server_detection.md** - Removed PostgreSQL/Redis
   - Updated infrastructure stack to Durable Objects + L1 cache
   - Converted multi-tier cache to L1-only (Moka)
   - Added dual verification to DetectionContext
   - Updated dependencies (removed sqlx, redis; added ed25519-dalek, blake3)

3. ✅ **005_008_action_token_impl.md** - Created comprehensive handover
   - 1,037 lines of detailed documentation
   - 5-step verification process documented
   - Testing procedures included
   - Migration path provided

---

## Pending Updates ⏳

### 1. README.md (CRITICAL)

**File:** `plans/008_xigncode3_impl/README.md`

**Why Critical:** This is the entry point for the entire implementation plan. It must accurately reflect the dual verification architecture before any coding begins.

**Changes Needed:**

#### Section: High-Level Flow (around line 50-80)
```markdown
# CURRENT:
POST /cheat (cheat event) → Cloudflare Worker → Durable Object

# SHOULD BE:
POST /register → Worker → Session DO (Ed25519 registration)
POST /token → Worker → Session DO (BLAKE3 token gen)
POST /cheat (signed payload + token) → Worker (5-step verification) → Session DO
```

#### Section: Key Components > Client Side (around line 110-150)
- Add emphasis on dual verification flow
- Include action token request sequence
- Show how callback signature includes action_token

#### Section: Key Components > Cloudflare Worker (around line 150-220)
- Add `/register` endpoint (POST with player_id, public_key)
- Add `/token` endpoint (POST with player_id, nonce)
- Update `/cheat` endpoint to show signed payload
- Include 5-step verification description

#### Section: Key Components > Durable Object (around line 220-280)
- Split into Session DO and Nonce Tracker DO
- Add SQLite schema: `players (player_id, public_key, registered_at)`
- Add nonce tracking mechanism description

---

### 2. TASKS.md (CRITICAL)

**File:** `plans/008_xigncode3_impl/TASKS.md`

**Why Critical:** Implementation checklist must be accurate. Outdated tasks will lead to wasted effort.

**Changes Needed:**

#### Section: Implementation Phases
```markdown
# Add Phase 0 (1-2 days):
- Complete documentation updates
- Review all plan documents for consistency
- Validate architecture diagrams

# Update Phase 1 (Week 1-2):
- Remove PostgreSQL/Redis setup tasks
- Add Durable Objects configuration tasks
- Add dual verification implementation tasks
- Add Ed25519 + BLAKE3 implementation

# Update Phase 2 (Week 3-4):
- Remove Axum backend tasks (deferred)
- Remove Docker container tasks (optional)
- Add Cloudflare Workers deployment tasks
- Add KV namespace setup tasks
```

#### Section: Task Dependencies
- Dual verification is prerequisite for:
  - All server-side detection tasks
  - Ban enforcement tasks
  - All testing tasks
- Remove dependencies on PostgreSQL/Redis setup

#### Section: Testing Checklist
```markdown
# Add new tests:
- [ ] Ed25519 key pair generation
- [ ] Ed25519 signature verification
- [ ] BLAKE3 action token generation
- [ ] BLAKE3 hash verification
- [ ] Nonce uniqueness check (Bloom filter)
- [ ] Nonce LRU cache operations
- [ ] Replay attack prevention (full flow)
- [ ] 5-step dual verification
- [ ] Action token expiry handling
```

#### Section: Deployment Checklist
```markdown
# Remove:
- PostgreSQL database setup
- Redis cluster setup
- Docker container deployment (optional)

# Add:
- [ ] Cloudflare Worker deployment
- [ ] Durable Objects configuration
- [ ] KV namespace creation
- [ ] Environment variables (SERVER_SECRET)
- [ ] Worker bindings in wrangler.toml
```

---

### 3. 008b_client_server_comm.md (HIGH PRIORITY)

**File:** `plans/008_xigncode3_impl/008b_client_server_comm.md`

**Why Important:** This document details the client-server communication layer. It must accurately reflect the dual verification protocol.

**Changes Needed:**

#### Section: Protocol Overview
```markdown
# Update to show three endpoints:
1. POST /register - First-time player registration
2. POST /token - Request action token
3. POST /cheat - Submit signed event with action token
```

#### Section: Request/Response Formats
- Add `/register` endpoint spec (player_id, public_key → success)
- Add `/token` endpoint spec (player_id, nonce → action_token)
- Update `/cheat` endpoint to include signature and action_token

#### Section: Security Flow
- Add 5-step verification process diagram
- Include nonce tracking flow
- Show error responses for replay attacks

---

### 4. 008e_ban_management.md (HIGH PRIORITY)

**File:** `plans/008_xigncode3_impl/008e_ban_management.md`

**Why Important:** Replay attacks should trigger automatic bans. This document needs to reflect that.

**Changes Needed:**

#### Section: Infrastructure Stack
```markdown
# REPLACE:
- PostgreSQL (via libsql): Persistent ban storage
- Redis: Ban status cache

# WITH:
- Durable Objects (SQLite): Ban storage and enforcement
- Cloudflare KV: Ban status cache for fast lookups
- External Systems: Ban enforcement (game servers, auth systems)
```

#### Section: Ban Types
- Add `ReplayAttack { nonce: String }` to BanType enum
- Document automatic banning for nonce reuse

#### Section: Database Schema
- Convert PostgreSQL to SQLite syntax:
  - `VARCHAR(36)` → `TEXT`
  - `JSONB` → `TEXT` (store JSON as string)
  - `BIGINT` → `INTEGER`
  - Remove `REFERENCES` clauses
- Add `nonce_violations` table:
  ```sql
  CREATE TABLE nonce_violations (
      violation_id TEXT PRIMARY KEY,
      player_id TEXT NOT NULL,
      nonce TEXT NOT NULL,
      detected_at INTEGER NOT NULL,
      action_token TEXT NOT NULL
  );
  ```

---

### 5. 008d_pattern_management.md (MEDIUM PRIORITY)

**File:** `plans/008_xigncode3_impl/008d_pattern_management.md`

**Why Less Critical:** Pattern management is important but can work with the existing architecture once converted.

**Changes Needed:**

#### Section: Infrastructure Stack
```markdown
# REPLACE:
- Axum: Backend API for pattern CRUD
- PostgreSQL: Persistent pattern storage
- Redis: Pattern cache

# WITH:
- Durable Objects (SQLite): Pattern storage and versioning
- Cloudflare KV: Pattern cache for clients
- WebSocket: Real-time distribution
```

#### Section: Database Schema (around line 130-180)
Convert all PostgreSQL syntax to SQLite:
- `VARCHAR(36)` → `TEXT`
- `BYTEA` → `BLOB`
- `JSONB` → `TEXT`
- `BIGINT` → `INTEGER`
- Remove all `INDEX` statements (SQLite auto-creates for PRIMARY KEY)
- Remove `REFERENCES table(id)` clauses (handle in code)

#### Section: Pattern Extraction
- Replace `sqlx::PgPool` with Durable Object SQLite access
- Update function signatures to use SQLite queries

---

### 6. 008f_analytics_monitoring.md (LOW PRIORITY)

**File:** `plans/008_xigncode3_impl/008f_analytics_monitoring.md`

**Why Low Priority:** Analytics is explicitly deferred to external systems in the new architecture. Only minimal changes needed.

**Changes Needed:**

#### Section: Infrastructure Stack
```markdown
# REPLACE:
- Axum: Backend API for analytics
- PostgreSQL: Persistent metrics storage
- Redis: Real-time metrics cache

# WITH:
- Durable Objects (SQLite): Short-term aggregation (< 24h)
- Cloudflare KV: Metrics cache for dashboards
- External Systems: Long-term analytics (PostgreSQL, Grafana, Prometheus)
```

#### Section: Data Pipeline
```markdown
# SIMPLIFIED:
Telemetry Events → Durable Object (aggregate) → Cloudflare KV (cache) → External Systems
```

#### Section: Metrics Schema
- Keep only essential DO metrics:
  - action_token_requests
  - nonce_reuse_count
  - signature_failures
  - token_failures
- Remove complex aggregation (deferred to external systems)

#### Section: Database Schema
- Convert to SQLite syntax
- Remove `aggregated_metrics` table (external systems handle this)
- Keep only basic metrics tables

---

## Implementation Priority

### Must Complete Before Coding
1. **README.md** - Architecture overview is the foundation
2. **TASKS.md** - Implementation checklist drives development
3. **008b_client_server_comm.md** - Protocol must be defined

### Complete During Week 1
4. **008c_server_detection.md** - Partially done, needs completion
5. **008e_ban_management.md** - Replay attack handling is critical

### Can Defer Until Week 2
6. **008d_pattern_management.md** - Pattern system is secondary
7. **008f_analytics_monitoring.md** - Analytics is external anyway

---

## Quick Reference: SQLite vs PostgreSQL Syntax

| PostgreSQL | SQLite | Notes |
|------------|--------|-------|
| `VARCHAR(36)` | `TEXT` | SQLite uses dynamic typing |
| `BYTEA` | `BLOB` | Binary data storage |
| `JSONB` | `TEXT` | Store as JSON string, parse in app |
| `BIGINT` | `INTEGER` | SQLite integers are variable-width |
| `BOOLEAN` | `INTEGER` | 0 = false, 1 = true |
| `REFERENCES table(id)` | (remove) | Handle in application layer |
| `CREATE INDEX` | (remove for PK) | Auto-created for PRIMARY KEY |
| `AUTO INCREMENT` | `AUTOINCREMENT` | SQLite syntax |
| `TIMESTAMP` | `INTEGER` | Use epoch seconds |
| `CURRENT_TIMESTAMP` | `(strftime('%s', 'now'))` | SQLite timestamp function |

---

## Validation Checklist

After completing all updates, verify:

- [ ] All architecture diagrams are consistent across documents
- [ ] Dual verification flow is documented in at least 3 places
- [ ] No PostgreSQL/Redis references remain (except in external system sections)
- [ ] Durable Object SQLite is mentioned in all database sections
- [ ] Cloudflare KV is consistently the caching layer
- [ ] External systems are clearly defined for deferred components
- [ ] TASKS.md checklist aligns with updated architecture
- [ ] All code examples use appropriate Rust syntax (Workers/DO)
- [ ] No conflicting information between documents
- [ ] Player ID derivation (BLAKE3 of Ed25519 public_key) is consistent

---

## Notes

1. **API Rate Limit:** These updates were tracked separately due to API rate limits during initial handover creation.

2. **Simplicity Principle:** The dual verification system intentionally removes complexity. All documents should reflect this simpler architecture.

3. **External Systems:** Historical data, complex analytics, and ML processing are explicitly deferred to external systems. Do not implement these in the anti-cheat service.

4. **Consistency:** ARCHITECTURE_SUMMARY.md v3.0 is the source of truth. When in doubt, align with that document.

5. **Incremental Updates:** It's acceptable to implement code in parallel with documentation updates, as long as README.md and TASKS.md are accurate first.

---

## Next Steps

### Option A: Complete All Updates First (Recommended)
- **Time:** 1-2 hours
- **Pros:** Clear direction, no confusion, minimal rework
- **Cons:** Delays coding slightly

### Option B: Critical Updates Only
- **Files:** README.md, TASKS.md, 008b_client_server_comm.md
- **Time:** 30-45 minutes
- **Pros:** Faster start to coding
- **Cons:** May need to revisit other docs later

### Option C: Start Coding Now
- **Approach:** Update docs as needed during implementation
- **Pros:** Immediate progress
- **Cons:** Risk of implementing wrong architecture, potential rework

**Recommendation:** Option A - Complete all critical and high-priority updates before Week 1 begins.

---

## Contacts

For questions about these updates:
- Reference: ARCHITECTURE_SUMMARY.md v3.0
- Handover: 005_008_action_token_impl.md
- Implementation Plan: README.md (after update)

---

**Created by:** AI Assistant (GLM 4.7)  
**Date:** 2025-01-27  
**Status:** Pending completion  
**Estimated Time:** 1-2 hours for full completion