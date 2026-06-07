# 008d: Pattern Management System

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, Security Engineers, Data Scientists |

## Overview
This plan implements a pattern management system that creates, validates, distributes, and maintains security patterns used for anti-cheat detection. Patterns are derived from detection logs, tested for accuracy, and distributed to clients in real-time via WebSocket connections.

## Architecture Notes

### Infrastructure Stack
- **Cloudflare Workers**: Pattern validation and preprocessing
- **Cloudflare Durable Objects**: Pattern version management and distribution state
- **Durable Objects (SQLite)**: Pattern storage, version history, distribution tracking
- **Cloudflare KV**: Pattern cache for fast client lookups
- **WebSocket**: Real-time pattern distribution to clients

### Pattern Lifecycle
```
Log Collection
  → Pattern Extraction (ML/Heuristic)
    → Manual Validation (Admin Review)
      → Pattern Testing (Staging)
        → Pattern Approval
          → Versioned Release
            → Distribution (WebSocket)
              → Client Application
                → Feedback Loop (Effectiveness Metrics)
```

### Pattern Types
1. **Signature Patterns**: Known cheat signatures (hashes, byte patterns)
2. **Behavioral Patterns**: Heuristic rules for suspicious behavior
3. **Network Patterns**: IP/Port signatures, protocol anomalies
4. **Timing Patterns**: Input timing signatures for macro detection
5. **Fingerprint Patterns**: System configuration signatures

## Implementation Tasks

### Task 1: Pattern Schema Design (Day 1-2)

#### 1.1 Core Pattern Structure (types.rs)
```rust
// src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub version: u32,
    pub signature: Vec<u8>,          // Binary signature or rule
    pub metadata: PatternMetadata,
    pub effectiveness: EffectivenessMetrics,
    pub status: PatternStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Signature {
        module_name: String,
        offset: Option<u64>,
        length: usize,
    },
    Behavioral {
        rule: String,
        threshold: f32,
        conditions: HashMap<String, serde_json::Value>,
    },
    Network {
        ip_range: String,
        port: Option<u16>,
        protocol: String,
    },
    Timing {
        interval_mean: f64,
        interval_variance: f64,
        tolerance: f32,
    },
    Fingerprint {
        os_version: String,
        hardware_config: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMetadata {
    pub name: String,
    pub description: String,
    pub threat_level: ThreatLevel,
    pub author: String,
    pub source: PatternSource,
    pub tags: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternSource {
    Automated,
    Manual,
    CommunityReport,
    ThirdParty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessMetrics {
    pub total_matches: u64,
    pub false_positives: u32,
    pub true_positives: u32,
    pub precision: f32,
    pub recall: f32,
    pub f1_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternStatus {
    Draft,
    Testing,
    Approved,
    Active,
    Deprecated,
    Rejected,
}
```

#### 1.2 Database Schema
```sql
-- patterns table
CREATE TABLE patterns (
    pattern_id VARCHAR(36) PRIMARY KEY,
    pattern_type VARCHAR(50) NOT NULL,
    version INTEGER NOT NULL,
    signature BYTEA NOT NULL,
    metadata JSONB NOT NULL,
    effectiveness JSONB NOT NULL,
    status VARCHAR(20) NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    expires_at BIGINT
);

-- pattern_distribution table
CREATE TABLE pattern_distribution (
    distribution_id VARCHAR(36) PRIMARY KEY,
    pattern_id VARCHAR(36) NOT NULL REFERENCES patterns(pattern_id),
    distributed_at BIGINT NOT NULL,
    distribution_status VARCHAR(20) NOT NULL,
    affected_clients INTEGER DEFAULT 0,
    UNIQUE(pattern_id, distributed_at)
);

-- pattern_feedback table
CREATE TABLE pattern_feedback (
    feedback_id VARCHAR(36) PRIMARY KEY,
    pattern_id VARCHAR(36) NOT NULL REFERENCES patterns(pattern_id),
    player_id VARCHAR(36) NOT NULL,
    is_false_positive BOOLEAN NOT NULL,
    timestamp BIGINT NOT NULL,
    context JSONB,
    INDEX idx_pattern_feedback_pattern (pattern_id),
    INDEX idx_pattern_feedback_timestamp (timestamp)
);

-- pattern_history table (audit trail)
CREATE TABLE pattern_history (
    history_id VARCHAR(36) PRIMARY KEY,
    pattern_id VARCHAR(36) NOT NULL,
    action VARCHAR(20) NOT NULL,
    previous_status VARCHAR(20),
    new_status VARCHAR(20),
    changed_by VARCHAR(50) NOT NULL,
    timestamp BIGINT NOT NULL,
    changes JSONB,
    INDEX idx_pattern_history_pattern (pattern_id)
);
```

### Task 2: Pattern Extraction from Logs (Day 2-4)

#### 2.1 Log Analyzer (extraction.rs)
```rust
// src/extraction.rs
use crate::types::{SecurityPattern, PatternType, PatternMetadata, PatternSource};
use sqlx::PgPool;
use blake3::Hash;
use uuid::Uuid;
use anyhow::Result;

pub struct PatternExtractor {
    db_pool: PgPool,
    extraction_rules: Vec<ExtractionRule>,
}

#[derive(Debug, Clone)]
pub struct ExtractionRule {
    pub rule_id: String,
    pub pattern_type: PatternType,
    pub trigger_conditions: Vec<String>,
    pub sample_size: u32,
    pub confidence_threshold: f32,
}

impl PatternExtractor {
    pub fn new(db_pool: PgPool) -> Self {
        PatternExtractor {
            db_pool,
            extraction_rules: Self::default_rules(),
        }
    }

    pub async fn extract_patterns_from_logs(
        &self,
        time_range: (u64, u64),
    ) -> Result<Vec<SecurityPattern>> {
        let mut candidates = Vec::new();

        // Query detection logs for the time range
        let logs = self.query_detection_logs(time_range).await?;

        for log in logs {
            // Apply extraction rules
            for rule in &self.extraction_rules {
                if let Some(candidate) = self.apply_extraction_rule(rule, &log)? {
                    candidates.push(candidate);
                }
            }
        }

        // Deduplicate similar patterns
        let deduplicated = self.deduplicate_patterns(candidates)?;

        // Validate candidates
        let validated = self.validate_candidates(deduplicated).await?;

        Ok(validated)
    }

    async fn query_detection_logs(&self, range: (u64, u64)) -> Result<Vec<DetectionLog>> {
        let rows = sqlx::raw_sql(&format!(
            "SELECT event_id, player_id, event_type, timestamp, details, context \
             FROM detection_logs \
             WHERE timestamp BETWEEN {} AND {} \
             AND threat_level IN ('High', 'Critical') \
             ORDER BY timestamp DESC \
             LIMIT 1000",
            range.0, range.1
        ))
        .fetch_all(&self.db_pool)
        .await?;

        // Convert rows to DetectionLog structs
        rows.into_iter()
            .map(|row| self.row_to_log(row))
            .collect()
    }

    fn apply_extraction_rule(
        &self,
        rule: &ExtractionRule,
        log: &DetectionLog,
    ) -> Result<Option<SecurityPattern>> {
        // Check if log matches rule conditions
        if !self.check_conditions(&rule.trigger_conditions, log)? {
            return Ok(None);
        }

        // Extract pattern based on type
        let pattern_type = self.extract_pattern_type(rule, log)?;
        let signature = self.extract_signature(log)?;
        let metadata = PatternMetadata {
            name: format!("Auto-generated from log {}", log.event_id),
            description: "Automatically extracted from detection logs".to_string(),
            threat_level: log.threat_level,
            author: "System".to_string(),
            source: PatternSource::Automated,
            tags: vec!["auto-generated".to_string()],
            references: vec![log.event_id.clone()],
        };

        Ok(Some(SecurityPattern {
            pattern_id: Uuid::new_v4().to_string(),
            pattern_type,
            version: 1,
            signature,
            metadata,
            effectiveness: EffectivenessMetrics::default(),
            status: PatternStatus::Draft,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
            expires_at: None,
        }))
    }

    fn deduplicate_patterns(&self, patterns: Vec<SecurityPattern>) -> Result<Vec<SecurityPattern>> {
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for pattern in patterns {
            let hash = Hash::hash(&pattern.signature);
            if seen.insert(hash) {
                unique.push(pattern);
            }
        }

        Ok(unique)
    }

    async fn validate_candidates(&self, patterns: Vec<SecurityPattern>) -> Result<Vec<SecurityPattern>> {
        // Validate patterns against known false positives
        // Check pattern complexity
        // Verify signature format
        
        patterns.into_iter()
            .filter(|p| self.is_valid_pattern(p))
            .collect()
    }

    fn is_valid_pattern(&self, pattern: &SecurityPattern) -> bool {
        // Validate signature not empty
        if pattern.signature.is_empty() {
            return false;
        }

        // Validate pattern type constraints
        match &pattern.pattern_type {
            PatternType::Signature { module_name, .. } => !module_name.is_empty(),
            PatternType::Behavioral { rule, .. } => !rule.is_empty(),
            PatternType::Network { ip_range, .. } => !ip_range.is_empty(),
            PatternType::Timing { interval_mean, .. } => *interval_mean > 0.0,
            PatternType::Fingerprint { os_version, .. } => !os_version.is_empty(),
        }
    }

    fn default_rules() -> Vec<ExtractionRule> {
        vec![
            ExtractionRule {
                rule_id: "macro_timing".to_string(),
                pattern_type: PatternType::Timing {
                    interval_mean: 0.0,
                    interval_variance: 0.0,
                    tolerance: 0.1,
                },
                trigger_conditions: vec![
                    "event_type=timing_anomaly".to_string(),
                    "jitter<1.0".to_string(),
                ],
                sample_size: 50,
                confidence_threshold: 0.85,
            },
            ExtractionRule {
                rule_id: "known_cheat".to_string(),
                pattern_type: PatternType::Signature {
                    module_name: String::new(),
                    offset: None,
                    length: 0,
                },
                trigger_conditions: vec![
                    "event_type=process_injection".to_string(),
                    "known_cheat=true".to_string(),
                ],
                sample_size: 1,
                confidence_threshold: 0.95,
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct DetectionLog {
    pub event_id: String,
    pub player_id: String,
    pub event_type: String,
    pub timestamp: u64,
    pub details: serde_json::Value,
    pub threat_level: ThreatLevel,
}
```

### Task 3: Pattern Validation System (Day 4-6)

#### 3.1 Validation Engine (validation.rs)
```rust
// src/validation.rs
use crate::types::{SecurityPattern, PatternStatus, EffectivenessMetrics};
use sqlx::PgPool;
use anyhow::Result;

pub struct PatternValidator {
    db_pool: PgPool,
    test_samples: Vec<TestSample>,