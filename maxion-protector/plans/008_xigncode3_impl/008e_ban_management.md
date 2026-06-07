# 008e: Ban Management System

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, Security Engineers, Game Moderators |

Detection Result
  → Threat Score Evaluation
    → Ban Rule Matching
      → Ban History Check
        → Whitelist Validation
          → Ban Application
            → Notification System
              → Audit Logging
```

### Ban Types
1. **Temporary Ban**: Time-limited (hours/days)
2. **Permanent Ban**: Indefinite
3. **HWID Ban**: Hardware ID based
4. **IP Ban**: IP address/range based
5. **Shadow Ban**: Restrictions without notification

## Implementation Tasks

### Task 1: Ban Schema Design (Day 1-2)

#### 1.1 Core Ban Types (types.rs)
```rust
// src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BanType {
    Temporary { duration_seconds: u64 },
    Permanent,
    HardwareId,
    IpRange { cidr: String },
    Shadow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BanReason {
    AutomatedThreat { threat_score: f32, flags: Vec<String> },
    ManualReview { reason: String, admin_id: String },
    CommunityReport { report_count: u32 },
    ViolationHistory { severity: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BanStatus {
    Active,
    Expired,
    Lifted,
    UnderReview,
    Appealed,
}
```

#### 1.2 Ban Record Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRecord {
    pub ban_id: String,
    pub player_id: String,
    pub ban_type: BanType,
    pub reason: BanReason,
    pub status: BanStatus,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub created_by: String,
    pub metadata: BanMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanMetadata {
    pub hardware_id: Option<String>,
    pub ip_address: Option<String>,
    pub game_sessions_affected: u32,
    pub evidence_urls: Vec<String>,
    pub notes: String,
}
```

### Task 2: Ban Evaluation Logic (Day 2-4)

#### 2.1 Ban Rule Engine
```rust
// src/engine.rs
use crate::types::*;

pub struct BanEngine {
    rules: Vec<BanRule>,
}

#[derive(Debug, Clone)]
pub struct BanRule {
    pub rule_id: String,
    pub condition: BanCondition,
    pub action: BanAction,
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub enum BanCondition {
    ThresholdExceeded { metric: String, value: f32 },
    PatternMatched { pattern_id: String },
    Flagged { flag: String },
    TimeWindowExceeded { metric: String, window_seconds: u64, count: u32 },
}

#[derive(Debug, Clone)]
pub enum BanAction {
    TemporaryBan { duration_seconds: u64 },
    PermanentBan,
    ShadowBan,
    FlagForReview,
}

impl BanEngine {
    pub fn new() -> Self {
        BanEngine {
            rules: vec![
                // Default rules
                BanRule {
                    rule_id: "critical_threat".to_string(),
                    condition: BanCondition::ThresholdExceeded {
                        metric: "threat_score".to_string(),
                        value: 0.9,
                    },
                    action: BanAction::PermanentBan,
                    priority: 1,
                },
                BanRule {
                    rule_id: "high_threat".to_string(),
                    condition: BanCondition::ThresholdExceeded {
                        metric: "threat_score".to_string(),
                        value: 0.7,
                    },
                    action: BanAction::TemporaryBan {
                        duration_seconds: 86400, // 24 hours
                    },
                    priority: 2,
                },
            ],
        }
    }

    pub fn evaluate(&self, detection: &DetectionResult) -> Option<BanAction> {
        // Sort rules by priority
        let mut sorted_rules = self.rules.clone();
        sorted_rules.sort_by_key(|r| r.priority);

        for rule in &sorted_rules {
            if self.condition_matches(&rule.condition, detection) {
                return Some(rule.action.clone());
            }
        }
        None
    }

    fn condition_matches(&self, condition: &BanCondition, detection: &DetectionResult) -> bool {
        match condition {
            BanCondition::ThresholdExceeded { metric, value } => {
                if metric == "threat_score" {
                    detection.risk_score > *value
                } else {
                    false
                }
            }
            BanCondition::PatternMatched { pattern_id } => {
                detection.events.iter().any(|e| {
                    if let DetectionEvent::PatternMatched { pattern, .. } = e {
                        pattern == pattern_id
                    } else {
                        false
                    }
                })
            }
            BanCondition::Flagged { flag } => {
                detection.flags.contains(flag)
            }
            BanCondition::TimeWindowExceeded { metric, window_seconds, count } => {
                // Check if metric exceeded count times within window
                detection.time_series.get(metric)
                    .map(|entries| {
                        let now = chrono::Utc::now().timestamp();
                        let recent_count = entries.iter()
                            .filter(|t| now - t <= *window_seconds as i64)
                            .count();
                        recent_count >= *count as usize
                    })
                    .unwrap_or(false)
            }
        }
    }
}
```

### Task 3: Ban Service Implementation (Day 4-6)

#### 3.1 Ban Service
```rust
// src/service.rs
use crate::types::*;
use sqlx::PgPool;
use anyhow::Result;
use chrono::Utc;

pub struct BanService {
    pool: PgPool,
    engine: BanEngine,
}

impl BanService {
    pub fn new(pool: PgPool) -> Self {
        BanService {
            pool,
            engine: BanEngine::new(),
        }
    }

    pub async fn process_detection(&self, detection: &DetectionResult) -> Result<Option<BanRecord>> {
        // Check whitelist first
        if self.is_whitelisted(&detection.player_id).await? {
            return Ok(None);
        }

        // Check existing ban
        if let Some(existing_ban) = self.get_active_ban(&detection.player_id).await? {
            return Ok(Some(existing_ban));
        }

        // Evaluate ban rules
        if let Some(action) = self.engine.evaluate(detection) {
            return self.apply_ban(&detection.player_id, &action, detection).await.map(Some);
        }

        Ok(None)
    }

    pub async fn apply_ban(
        &self,
        player_id: &str,
        action: &BanAction,
        detection: &DetectionResult,
    ) -> Result<BanRecord> {
        let now = Utc::now().timestamp();
        
        let ban_record = BanRecord {
            ban_id: uuid::Uuid::new_v4().to_string(),
            player_id: player_id.to_string(),
            ban_type: match action {
                BanAction::TemporaryBan { duration_seconds } => {
                    BanType::Temporary {
                        duration_seconds: *duration_seconds,
                    }
                }
                BanAction::PermanentBan => BanType::Permanent,
                BanAction::ShadowBan => BanType::Shadow,
                BanAction::FlagForReview => {
                    return Err(anyhow::anyhow!("FlagForReview is not a ban type"));
                }
            },
            reason: BanReason::AutomatedThreat {
                threat_score: detection.risk_score,
                flags: detection.flags.clone(),
            },
            status: BanStatus::Active,
            created_at: now,
            expires_at: match action {
                BanAction::TemporaryBan { duration_seconds } => Some(now + *duration_seconds as i64),
                _ => None,
            },
            created_by: "system".to_string(),
            metadata: BanMetadata {
                hardware_id: detection.hardware_id.clone(),
                ip_address: detection.ip_address.clone(),
                game_sessions_affected: 0,
                evidence_urls: vec![],
                notes: format!("Auto-ban based on threat score: {}", detection.risk_score),
            },
        };

        self.save_ban(&ban_record).await?;
        
        // Notify other systems
        self.notify_ban_applied(&ban_record).await?;

        Ok(ban_record)
    }

    pub async fn save_ban(&self, ban: &BanRecord) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO bans (ban_id, player_id, ban_type, reason, status, 
                            created_at, expires_at, created_by, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            ban.ban_id,
            ban.player_id,
            serde_json::to_value(&ban.ban_type)?,
            serde_json::to_value(&ban.reason)?,
            serde_json::to_value(&ban.status)?,
            ban.created_at,
            ban.expires_at,
            ban.created_by,
            serde_json::to_value(&ban.metadata)?
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_active_ban(&self, player_id: &str) -> Result<Option<BanRecord>> {
        let now = Utc::now().timestamp();

        let row = sqlx::query!(
            r#"
            SELECT * FROM bans
            WHERE player_id = $1
              AND status = 'Active'
              AND (expires_at IS NULL OR expires_at > $2)
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            player_id,
            now
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| BanRecord {
            ban_id: r.ban_id,
            player_id: r.player_id,
            ban_type: serde_json::from_value(r.ban_type)?,
            reason: serde_json::from_value(r.reason)?,
            status: serde_json::from_value(r.status)?,
            created_at: r.created_at,
            expires_at: r.expires_at,
            created_by: r.created_by,
            metadata: serde_json::from_value(r.metadata)?,
        }))
    }

    pub async fn is_whitelisted(&self, player_id: &str) -> Result<bool> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count FROM whitelist
            WHERE player_id = $1
              AND (expires_at IS NULL OR expires_at > $2)
            "#,
            player_id,
            Utc::now().timestamp()
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0);

        Ok(count > 0)
    }

    pub async fn notify_ban_applied(&self, ban: &BanRecord) -> Result<()> {
        // TODO: Send notifications to:
        // - Game server (kick player)
        // - Analytics system (log event)
        // - Moderation dashboard (show alert)
        Ok(())
    }
}
```

### Task 4: Database Schema (Day 6-7)

#### 4.1 Schema Definition
```sql
-- Bans table
CREATE TABLE bans (
    ban_id VARCHAR(36) PRIMARY KEY,
    player_id VARCHAR(36) NOT NULL,
    ban_type JSONB NOT NULL,
    reason JSONB NOT NULL,
    status JSONB NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT,
    created_by VARCHAR(100) NOT NULL,
    metadata JSONB NOT NULL
);

CREATE INDEX idx_bans_player_id ON bans(player_id);
CREATE INDEX idx_bans_status ON bans(status);
CREATE INDEX idx_bans_expires_at ON bans(expires_at);

-- Whitelist table
CREATE TABLE whitelist (
    entry_id VARCHAR(36) PRIMARY KEY,
    player_id VARCHAR(36) NOT NULL,
    hardware_id VARCHAR(100),
    ip_address VARCHAR(45),
    reason TEXT NOT NULL,
    added_by VARCHAR(100) NOT NULL,
    expires_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_whitelist_player_id ON whitelist(player_id);
CREATE INDEX idx_whitelist_hardware_id ON whitelist(hardware_id);
CREATE INDEX idx_whitelist_ip_address ON whitelist(ip_address);

-- Ban appeals table
CREATE TABLE ban_appeals (
    appeal_id VARCHAR(36) PRIMARY KEY,
    ban_id VARCHAR(36) NOT NULL,
    player_id VARCHAR(36) NOT NULL,
    appeal_text TEXT NOT NULL,
    submitted_at BIGINT NOT NULL,
    status JSONB NOT NULL,
    reviewed_by VARCHAR(100),
    reviewed_at BIGINT,
    review_notes TEXT,
    
    FOREIGN KEY (ban_id) REFERENCES bans(ban_id),
    FOREIGN KEY (player_id) REFERENCES bans(player_id)
);

CREATE INDEX idx_appeals_ban_id ON ban_appeals(ban_id);
CREATE INDEX idx_appeals_player_id ON ban_appeals(player_id);
CREATE INDEX idx_appeals_status ON ban_appeals(status);
```

### Task 5: API Endpoints (Day 7-9)

#### 5.1 Ban Endpoints
```rust
// src/api.rs
use axum::{extract::Path, Json, Router};
use serde::{Deserialize, Serialize};

pub fn ban_routes() -> Router {
    Router::new()
        .route("/bans/:player_id", axum::routing::get(get_ban_status))
        .route("/bans/:player_id/lift", axum::routing::post(lift_ban))
        .route("/bans/:player_id/whitelist", axum::routing::post(add_to_whitelist))
        .route("/bans/:player_id/whitelist", axum::routing::delete(remove_from_whitelist))
        .route("/bans/appeals", axum::routing::post(submit_appeal))
        .route("/bans/appeals/:appeal_id", axum::routing::put(review_appeal))
}

#[derive(Serialize)]
struct BanStatusResponse {
    is_banned: bool,
    ban_record: Option<BanRecord>,
    is_whitelisted: bool,
}

async fn get_ban_status(
    Path(player_id): Path<String>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<BanStatusResponse>, AppError> {
    let ban_record = service.get_active_ban(&player_id).await?;
    let is_whitelisted = service.is_whitelisted(&player_id).await?;

    Ok(Json(BanStatusResponse {
        is_banned: ban_record.is_some(),
        ban_record,
        is_whitelisted,
    }))
}

#[derive(Deserialize)]
struct LiftBanRequest {
    admin_id: String,
    reason: String,
}

async fn lift_ban(
    Path(player_id): Path<String>,
    Json(req): Json<LiftBanRequest>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<()>, AppError> {
    service.lift_ban(&player_id, &req.admin_id, &req.reason).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct WhitelistRequest {
    admin_id: String,
    reason: String,
    expires_at: Option<i64>,
    hardware_id: Option<String>,
    ip_address: Option<String>,
}

async fn add_to_whitelist(
    Path(player_id): Path<String>,
    Json(req): Json<WhitelistRequest>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<()>, AppError> {
    service.add_to_whitelist(&player_id, &req.admin_id, &req.reason, req.expires_at, 
                             req.hardware_id, req.ip_address).await?;
    Ok(Json(()))
}

async fn remove_from_whitelist(
    Path(player_id): Path<String>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<()>, AppError> {
    service.remove_from_whitelist(&player_id).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct AppealRequest {
    player_id: String,
    ban_id: String,
    appeal_text: String,
}

async fn submit_appeal(
    Json(req): Json<AppealRequest>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<BanAppeal>, AppError> {
    let appeal = service.submit_appeal(&req.player_id, &req.ban_id, &req.appeal_text).await?;
    Ok(Json(appeal))
}

#[derive(Deserialize)]
struct ReviewAppealRequest {
    admin_id: String,
    decision: AppealDecision,
    notes: String,
}

#[derive(Deserialize)]
#[serde(tag = "decision")]
enum AppealDecision {
    Approve,
    Deny,
}

async fn review_appeal(
    Path(appeal_id): Path<String>,
    Json(req): Json<ReviewAppealRequest>,
    State(service): State<Arc<BanService>>,
) -> Result<Json<()>, AppError> {
    match req.decision {
        AppealDecision::Approve => {
            service.approve_appeal(&appeal_id, &req.admin_id, &req.notes).await?;
        }
        AppealDecision::Deny => {
            service.deny_appeal(&appeal_id, &req.admin_id, &req.notes).await?;
        }
    }
    Ok(Json(()))
}
```

### Task 6: Notification System (Day 9-10)

#### 6.1 Event Notifications
```rust
// src/notifications.rs
use crate::types::*;
use tokio::sync::mpsc;

pub struct NotificationService {
    sender: mpsc::UnboundedSender<BanNotification>,
}

#[derive(Debug, Clone)]
pub enum BanNotification {
    BanApplied { ban: BanRecord },
    BanLifted { player_id: String, ban_id: String },
    AppealSubmitted { appeal: BanAppeal },
    AppealReviewed { appeal_id: String, decision: AppealDecision },
}

impl NotificationService {
    pub fn new() -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();

        // Spawn notification handler
        tokio::spawn(async move {
            while let Some(notification) = receiver.recv().await {
                match notification {
                    BanNotification::BanApplied { ban } => {
                        Self::notify_ban_applied(ban).await;
                    }
                    BanNotification::BanLifted { player_id, ban_id } => {
                        Self::notify_ban_lifted(player_id, ban_id).await;
                    }
                    BanNotification::AppealSubmitted { appeal } => {
                        Self::notify_appeal_submitted(appeal).await;
                    }
                    BanNotification::AppealReviewed { appeal_id, decision } => {
                        Self::notify_appeal_reviewed(appeal_id, decision).await;
                    }
                }
            }
        });

        NotificationService { sender }
    }

    pub fn send(&self, notification: BanNotification) {
        let _ = self.sender.send(notification);
    }

    async fn notify_ban_applied(ban: BanRecord) {
        // Send to game server
        // Send to analytics
        // Send to moderation dashboard
    }

    async fn notify_ban_lifted(player_id: String, ban_id: String) {
        // Notify relevant systems
    }

    async fn notify_appeal_submitted(appeal: BanAppeal) {
        // Notify moderators
    }

    async fn notify_appeal_reviewed(appeal_id: String, decision: AppealDecision) {
        // Notify player
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
    fn test_ban_rule_evaluation() {
        let engine = BanEngine::new();
        let detection = DetectionResult {
            player_id: "test".to_string(),
            risk_score: 0.95,
            events: vec![],
            flags: vec![],
            hardware_id: None,
            ip_address: None,
            time_series: HashMap::new(),
        };

        let action = engine.evaluate(&detection);
        assert!(matches!(action, Some(BanAction::PermanentBan)));
    }

    #[test]
    fn test_temporary_ban() {
        let engine = BanEngine::new();
        let detection = DetectionResult {
            player_id: "test".to_string(),
            risk_score: 0.75,
            events: vec![],
            flags: vec![],
            hardware_id: None,
            ip_address: None,
            time_series: HashMap::new(),
        };

        let action = engine.evaluate(&detection);
        assert!(matches!(action, Some(BanAction::TemporaryBan { .. })));
    }
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_ban_application() {
    let pool = setup_test_pool().await;
    let service = BanService::new(pool);
    
    let detection = create_test_detection();
    let ban = service.process_detection(&detection).await.unwrap();
    
    assert!(ban.is_some());
    assert_eq!(ban.as_ref().unwrap().player_id, detection.player_id);
}

#[tokio::test]
async fn test_whitelist_exemption() {
    let pool = setup_test_pool().await;
    let service = BanService::new(pool);
    
    // Add to whitelist
    service.add_to_whitelist(&"player1".to_string(), "admin", "test", None, None, None)
        .await.unwrap();
    
    let detection = create_test_detection();
    let ban = service.process_detection(&detection).await.unwrap();
    
    assert!(ban.is_none()); // Should not ban whitelisted player
}
```

## Performance Requirements

- **Ban Evaluation**: < 10ms
- **Ban Application**: < 50ms
- **Ban Status Query**: < 5ms
- **Whitelist Check**: < 2ms
- **Throughput**: 10,000 ban evaluations/second

## Security Considerations

### Ban Tampering Prevention
- All ban operations logged with audit trail
- Ban modifications require admin authentication
- Ban records immutable (can only create new status)

### Whitelist Security
- Whitelist additions require multiple admin approvals
- Whitelist entries auto-expire
- Whitelist modifications logged

### Appeal System
- Appeals cannot modify active bans
- Appeal decisions require admin authentication
- Appeal history maintained indefinitely

## Dependencies

### Required Crates
- `sqlx` - Database operations
- `anyhow` - Error handling
- `serde` - Serialization
- `chrono` - Time handling
- `uuid` - ID generation
- `tokio` - Async runtime
- `axum` - Web framework

### External Dependencies
- PostgreSQL database
- Notification system
- Game server API
- Moderation dashboard

## Deliverables

1. Ban evaluation engine with configurable rules
2. Ban service with full CRUD operations
3. Database schema and migrations
4. REST API endpoints for ban management
5. Notification system for ban events
6. Comprehensive test suite
7. Documentation and examples

## Next Steps

1. Implement ban rule engine with default rules
2. Create database schema and migrations
3. Implement ban service with PostgreSQL
4. Add REST API endpoints
5. Implement notification system
6. Write comprehensive tests
7. Deploy to production
8. Monitor and tune performance

## Notes

- Ban system is designed to be independent of other services
- All ban decisions are auditable and reversible
- Whitelist provides override mechanism for false positives
- Appeal system allows players to contest bans
- Ban types can be extended as needed

## Known Limitations

- Ban rules are currently code-based (not database-driven)
- No built-in GUI for ban management (requires separate dashboard)
- Notification system is stubbed (needs integration)
- No automated escalation for repeated offenses

## Migration Path

### From 008d to 008e
1. Import shared types from `maxion-detection-types`
2. Import detection results from 008c
3. Implement ban evaluation using detection results
4. Set up database schema
5. Integrate with existing services

### To 008f
1. Export ban events to analytics service
2. Provide metrics for monitoring
3. Set up dashboards for ban tracking