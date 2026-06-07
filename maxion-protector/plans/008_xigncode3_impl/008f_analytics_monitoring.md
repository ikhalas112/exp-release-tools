# 008f: Analytics & Monitoring Service

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-27 |
| Version | 1.0 |
| Complexity | Advanced |
| Time to Read | 20 minutes |
| Audience | Developers, Data Engineers, DevOps, Security Analysts |

## Overview
This plan implements a comprehensive analytics and monitoring service that tracks system performance, generates statistical reports, monitors key metrics, and provides real-time alerting. This component aggregates data from all other services and presents actionable insights through dashboards and automated notifications.

## Architecture Notes

### Infrastructure Stack
- **Cloudflare Workers**: Real-time metrics aggregation and preprocessing
- **Cloudflare Durable Objects**: Session state and live metrics storage
- **Axum**: Backend API for analytics queries
- **PostgreSQL (via libsql)**: Persistent metrics storage and historical data
- **Redis**: Real-time metrics cache and pub/sub for alerts
- **Grafana/Prometheus**: External monitoring integration (optional)

### Data Pipeline
```
Telemetry/Detection Events
  → Real-time Aggregation (Workers)
    → Metrics Calculation
      → Time-series Storage (PostgreSQL)
        → Alert Evaluation
          → Dashboard Queries
            → Report Generation
              → Data Export
```

### Metrics Categories
1. **System Metrics**: CPU, memory, network, latency
2. **Detection Metrics**: False positives, true positives, detection rates
3. **Ban Metrics**: Active bans, ban types, appeal success rates
4. **Performance Metrics**: Query times, cache hit rates, throughput
5. **Security Metrics**: Threat levels, pattern effectiveness, breach attempts

## Implementation Tasks

### Task 1: Metrics Schema Design (Day 1-2)

#### 1.1 Core Metric Types (types.rs)
```rust
// src/types.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub metric_id: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub timestamp: u64,
    pub tags: HashMap<String, String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    pub metric_name: String,
    pub aggregation: AggregationType,
    pub time_range: TimeRange,
    pub value: f64,
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub percentile_50: f64,
    pub percentile_95: f64,
    pub percentile_99: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Sum,
    Average,
    Min,
    Max,
    Count,
    Percentile(f32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub enabled: bool,
    pub notification_channels: Vec<NotificationChannel>,
    pub cooldown_seconds: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertCondition {
    pub operator: ComparisonOperator,
    pub threshold: f64,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    EqualTo,
    NotEqualTo,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub channel_type: ChannelType,
    pub destination: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelType {
    Email,
    Slack,
    Webhook,
    PagerDuty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_id: String,
    pub rule_id: String,
    pub triggered_at: u64,
    pub resolved_at: Option<u64>,
    pub status: AlertStatus,
    pub value: f64,
    pub threshold: f64,
    pub message: String,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertStatus {
    Firing,
    Resolved,
    Acknowledged,
}
```

#### 1.2 Database Schema
```sql
-- metrics table
CREATE TABLE metrics (
    metric_id VARCHAR(36) PRIMARY KEY,
    metric_type VARCHAR(20) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    timestamp BIGINT NOT NULL,
    tags JSONB,
    metadata JSONB,
    INDEX idx_metrics_name_timestamp (metric_name, timestamp),
    INDEX idx_metrics_timestamp (timestamp)
);

-- aggregated_metrics table (hourly/daily)
CREATE TABLE aggregated_metrics (
    agg_id VARCHAR(36) PRIMARY KEY,
    metric_name VARCHAR(100) NOT NULL,
    aggregation VARCHAR(20) NOT NULL,
    time_start BIGINT NOT NULL,
    time_end BIGINT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    count BIGINT NOT NULL,
    min_value DOUBLE PRECISION,
    max_value DOUBLE PRECISION,
    p50 DOUBLE PRECISION,
    p95 DOUBLE PRECISION,
    p99 DOUBLE PRECISION,
    UNIQUE(metric_name, aggregation, time_start, time_end)
);

-- alert_rules table
CREATE TABLE alert_rules (
    rule_id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    metric_name VARCHAR(100) NOT NULL,
    condition JSONB NOT NULL,
    severity VARCHAR(20) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    notification_channels JSONB NOT NULL,
    cooldown_seconds BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    INDEX idx_rules_enabled (enabled)
);

-- alerts table
CREATE TABLE alerts (
    alert_id VARCHAR(36) PRIMARY KEY,
    rule_id VARCHAR(36) NOT NULL REFERENCES alert_rules(rule_id),
    triggered_at BIGINT NOT NULL,
    resolved_at BIGINT,
    status VARCHAR(20) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    message TEXT,
    context JSONB,
    INDEX idx_alerts_rule (rule_id),
    INDEX idx_alerts_status (status),
    INDEX idx_alerts_timestamp (triggered_at)
);

-- reports table
CREATE TABLE reports (
    report_id VARCHAR(36) PRIMARY KEY,
    report_type VARCHAR(50) NOT NULL,
    parameters JSONB NOT NULL,
    generated_at BIGINT NOT NULL,
    generated_by VARCHAR(50),
    file_url TEXT,
    status VARCHAR(20) NOT NULL,
    expires_at BIGINT
);
```

### Task 2: Metrics Collection Service (Day 2-4)

#### 2.1 Metrics Collector (collector.rs)
```rust
// src/collector.rs
use crate::types::{Metric, MetricType, MetricValue};
use sqlx::PgPool;
use redis::AsyncCommands;
use uuid::Uuid;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MetricsCollector {
    db_pool: PgPool,
    buffer: Arc<RwLock<Vec<Metric>>>,
    buffer_size: usize,
    flush_interval_seconds: u64,
    redis_client: redis::Client,
}

impl MetricsCollector {
    pub fn new(db_pool: PgPool, redis_url: &str, buffer_size: usize) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        
        Ok(MetricsCollector {
            db_pool,
            buffer: Arc::new(RwLock::new(Vec::with_capacity(buffer_size))),
            buffer_size,
            flush_interval_seconds: 5,
            redis_client,
        })
    }

    pub async fn start_background_flush(&self) -> Result<()> {
        let buffer = self.buffer.clone();
        let db_pool = self.db_pool.clone();
        let flush_interval = self.flush_interval_seconds;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(flush_interval));
            
            loop {
                interval.tick().await;
                
                let mut buffer_guard = buffer.write().await;
                if !buffer_guard.is_empty() {
                    let metrics = std::mem::take(&mut *buffer_guard);
                    
                    if let Err(e) = Self::flush_metrics(&db_pool, metrics).await {
                        tracing::error!("Failed to flush metrics: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn record(&self, metric: Metric) -> Result<()> {
        let mut buffer = self.buffer.write().await;
        
        buffer.push(metric);
        
        if buffer.len() >= self.buffer_size {
            let metrics = std::mem::take(&mut *buffer);
            tokio::spawn(Self::flush_metrics(self.db_pool.clone(), metrics));
        }

        // Also publish to Redis for real-time monitoring
        self.publish_to_redis(&metric).await?;
        
        Ok(())
    }

    pub async fn increment_counter(&self, name: &str, value: i64, tags: HashMap<String, String>) -> Result<()> {
        let metric = Metric {
            metric_id: Uuid::now_v7().to_string(),
            metric_type: MetricType::Counter,
            metric_name: name.to_string(),
            value: MetricValue::Int(value),
            timestamp: current_timestamp(),
            tags,
            metadata: None,
        };
        
        self.record(metric).await
    }

    pub async fn record_gauge(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()> {
        let metric = Metric {
            metric_id: Uuid::now_v7().to_string(),
            metric_type: MetricType::Gauge,
            metric_name: name.to_string(),
            value: MetricValue::Float(value),
            timestamp: current_timestamp(),
            tags,
            metadata: None,
        };
        
        self.record(metric).await
    }

    pub async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>) -> Result<()> {
        let metric = Metric {
            metric_id: Uuid::now_v7().to_string(),
            metric_type: MetricType::Histogram,
            metric_name: name.to_string(),
            value: MetricValue::Float(value),
            timestamp: current_timestamp(),
            tags,
            metadata: None,
        };
        
        self.record(metric).await
    }

    async fn publish_to_redis(&self, metric: &Metric) -> Result<()> {
        let mut conn = self.redis_client.get_async_connection().await?;
        
        let channel = format!("metrics:{}", metric.metric_name);
        let message = serde_json::to_string(metric)?;
        
        conn.publish(&channel, message).await?;
        
        Ok(())
    }

    async fn flush_metrics(db_pool: &PgPool, metrics: Vec<Metric>) -> Result<()> {
        if metrics.is_empty() {
            return Ok(());
        }

        let mut transaction = db_pool.begin().await?;

        for metric in metrics {
            let value = match metric.value {
                MetricValue::Int(v) => v as f64,
                MetricValue::Float(v) => v,
                MetricValue::Bool(v) => if v { 1.0 } else { 0.0 },
                MetricValue::String(_) => 0.0,
            };

            sqlx::raw_sql(&format!(
                "INSERT INTO metrics (metric_id, metric_type, metric_name, value, timestamp, tags, metadata) \
                 VALUES ('{}', '{}', '{}', {}, {}, '{}', '{}')",
                metric.metric_id,
                format!("{:?}", metric.metric_type),
                metric.metric_name,
                value,
                metric.timestamp,
                serde_json::to_string(&metric.tags)?,
                metric.metadata.map(|m| serde_json::to_string(&m)).unwrap_or("NULL".to_string())
            ))
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        
        tracing::info!("Flushed {} metrics", metrics.len());
        
        Ok(())
    }
}
```

### Task 3: Aggregation Service (Day 4-6)

#### 3.1 Metrics Aggregator (aggregator.rs)
```rust
// src/aggregator.rs
use crate::types::{AggregatedMetric, AggregationType, TimeRange};
use sqlx::PgPool;
use anyhow::Result;

pub struct MetricsAggregator {
    db_pool: PgPool,
}

impl MetricsAggregator {
    pub fn new(db_pool: PgPool) -> Self {
        MetricsAggregator { db_pool }
    }

    pub async fn aggregate_metrics(
        &self,
        metric_name: &str,
        aggregation: AggregationType,
        time_range: TimeRange,
    ) -> Result<AggregatedMetric> {
        // Try to get from aggregated table first
        if let Some(agg) = self.get_cached_aggregation(metric_name, &aggregation, &time_range).await? {
            return Ok(agg);
        }

        // Calculate from raw metrics
        let rows = sqlx::raw_sql(&format!(
            "SELECT value, timestamp \
             FROM metrics \
             WHERE metric_name = '{}' \
             AND timestamp BETWEEN {} AND {} \
             ORDER BY timestamp",
            metric_name, time_range.start, time_range.end
        ))
        .fetch_all(&self.db_pool)
        .await?;

        let values: Vec<f64> = rows.iter()
            .map(|row| row.try_get::<f64, _>("value").unwrap_or(0.0))
            .collect();

        if values.is_empty() {
            return Err(anyhow::anyhow!("No metrics found for time range"));
        }

        let aggregated = self.calculate_aggregation(&values, aggregation)?;
        
        // Cache the result
        self.store_aggregation(metric_name, aggregation.clone(), time_range.clone(), &aggregated).await?;

        Ok(aggregated)
    }

    fn calculate_aggregation(&self, values: &[f64], aggregation: AggregationType) -> Result<AggregatedMetric> {
        let count = values.len() as u64;
        let sum: f64 = values.iter().sum();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        
        // Calculate percentiles
        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let p50 = self.calculate_percentile(&sorted_values, 50.0);
        let p95 = self.calculate_percentile(&sorted_values, 95.0);
        let p99 = self.calculate_percentile(&sorted_values, 99.0);

        let value = match aggregation {
            AggregationType::Sum => sum,
            AggregationType::Average => sum / count as f64,
            AggregationType::Min => min,
            AggregationType::Max => max,
            AggregationType::Count => count as f64,
            AggregationType::Percentile(p) => self.calculate_percentile(&sorted_values, p as f64),
        };

        Ok(AggregatedMetric {
            metric_name: String::new(),  // Set by caller
            aggregation,
            time_range: TimeRange { start: 0, end: 0 },  // Set by caller
            value,
            count,
            min,
            max,
            percentile_50: p50,
            percentile_95: p95,
            percentile_99: p99,
        })
    }

    fn calculate_percentile(&self, sorted_values: &[f64], percentile: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        
        let index = ((percentile / 100.0) * (sorted_values.len() - 1) as f64) as usize;
        sorted_values[index.min(sorted_values.len() - 1)]
    }

    async fn get_cached_aggregation(
        &self,
        metric_name: &str,
        aggregation: &AggregationType,
        time_range: &TimeRange,
    ) -> Result<Option<AggregatedMetric>> {
        let row = sqlx::raw_sql(&format!(
            "SELECT value, count, min_value, max_value, p50, p95, p99 \
             FROM aggregated_metrics \
             WHERE metric_name = '{}' \
             AND aggregation = '{}' \
             AND time_start = {} \
             AND time_end = {}",
            metric_name,
            serde_json::to_string(aggregation)?,
            time_range.start,
            time_range.end
        ))
        .fetch_optional(&self.db_pool)
        .await?;

        match row {
            Some(row) => Ok(Some(AggregatedMetric {
                metric_name: metric_name.to_string(),
                aggregation: aggregation.clone(),
                time_range: time_range.clone(),
                value: row.try_get("value")?,
                count: row.try_get("count")?,
                min: row.try_get("min_value")?,
                max: row.try_get("max_value")?,
                percentile_50: row.try_get("p50")?,
                percentile_95: row.try_get("p95")?,
                percentile_99: row.try_get("p99")?,
            })),
            None => Ok(None),
        }
    }

    async fn store_aggregation(
        &self,
        metric_name: &str,
        aggregation: AggregationType,
        time_range: TimeRange,
        aggregated: &AggregatedMetric,
    ) -> Result<()> {
        sqlx::raw_sql(&format!(
            "INSERT INTO aggregated_metrics (agg_id, metric_name, aggregation, time_start, time_end, value, count, min_value, max_value, p50, p95, p99) \
             VALUES ('{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}) \
             ON CONFLICT (metric_name, aggregation, time_start, time_end) \
             DO UPDATE SET value = EXCLUDED.value, count = EXCLUDED.count, min_value = EXCLUDED.min_value, max_value = EXCLUDED.max_value, p50 = EXCLUDED.p50, p95 = EXCLUDED.p95, p99 = EXCLUDED.p99",
            Uuid::now_v7().to_string(),
            metric_name,
            serde_json::to_string(&aggregation)?,
            time_range.start,
            time_range.end,
            aggregated.value,
            aggregated.count,
            aggregated.min,
            aggregated.max,
            aggregated.percentile_50,
            aggregated.percentile_95,
            aggregated.percentile_99
        ))
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    pub async fn periodic_aggregation(&self) -> Result<()> {
        // Aggregate metrics for the last hour
        let now = current_timestamp();
        let hour_start = now - 3600;
        let hour_end = now;

        let metric_names = self.get_unique_metric_names().await?;

        for metric_name in metric_names {
            let time_range = TimeRange {
                start: hour_start,
                end: hour_end,
            };

            // Calculate different aggregations
            for aggregation in &[
                AggregationType::Sum,
                AggregationType::Average,
                AggregationType::Min,
                AggregationType::Max,
                AggregationType::Count,
            ] {
                if let Err(e) = self.aggregate_metrics(&metric_name, aggregation.clone(), time_range).await {
                    tracing::warn!("Failed to aggregate {} for {}: {}", metric_name, serde_json::to_string(aggregation)?, e);
                }
            }
        }

        Ok(())
    }

    async fn get_unique_metric_names(&self) -> Result<Vec<String>> {
        let rows = sqlx::raw_sql(
            "SELECT DISTINCT metric_name FROM metrics WHERE timestamp > {} ORDER BY metric_name",
            current_timestamp() - 3600
        )
        .fetch_all(&self.db_pool)
        .await?;

        rows.into_iter()
            .map(|row| row.try_get("metric_name"))
            .collect()
    }
}
```

### Task 4: Alerting System (Day 6-8)

#### 4.1 Alert Manager (alert_manager.rs)
```rust
// src/alert_manager.rs
use crate::types::{AlertRule, Alert, AlertCondition, AlertSeverity, AlertStatus};
use sqlx::PgPool;
use redis::AsyncCommands;
use uuid::Uuid;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AlertManager {
    db_pool: PgPool,
    rules: Arc<RwLock<Vec<AlertRule>>>,
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    redis_client: redis::Client,
}

impl AlertManager {
    pub fn new(db_pool: PgPool, redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        
        Ok(AlertManager {
            db_pool,
            rules: Arc::new(RwLock::new(Vec::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
        })
    }

    pub async fn load_rules(&self) -> Result<()> {
        let rows = sqlx::raw_sql(
            "SELECT rule_id, name, description, metric_name, condition, severity, enabled, notification_channels, cooldown_seconds, created_at \
             FROM alert_rules WHERE enabled = true"
        )
        .fetch_all(&self.db_pool)
        .await?;

        let rules: Vec<AlertRule> = rows.into_iter()
            .map(|row| -> Result<AlertRule> {
                Ok(AlertRule {
                    rule_id: row.try_get("rule_id")?,
                    name: row.try_get("name")?,
                    description: row.try_get("description")?,
                    metric_name: row.try_get("metric_name")?,
                    condition: serde_json::from_str(row.try_get("condition")?)?,
                    severity: match row.try_get::<String, _>("severity")?.as_str() {
                        "Info" => AlertSeverity::Info,
                        "Warning" => AlertSeverity::Warning,
                        "Error" => AlertSeverity::Error,
                        "Critical" => AlertSeverity::Critical,
                        _ => return Err(anyhow::anyhow!("Invalid alert severity")),
                    },
                    enabled: row.try_get("enabled")?,
                    notification_channels: serde_json::from_str(row.try_get("notification_channels")?)?,
                    cooldown_seconds: row.try_get("cooldown_seconds")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        *self.rules.write().await = rules;
        
        Ok(())
    }

    pub async fn evaluate_rules(&self, metric_name: &str, value: f64) -> Result<Vec<Alert>> {
        let rules = self.rules.read().await;
        let mut triggered_alerts = Vec::new();
        let now = current_timestamp();

        for rule in rules.iter() {
            if rule.metric_name != metric_name {
                continue;
            }

            if self.should_trigger_alert(rule, value, now).await? {
                let alert = self.create_alert(rule, value).await?;
                triggered_alerts.push(alert);
            }
        }

        Ok(triggered_alerts)
    }

    async fn should_trigger_alert(&self, rule: &AlertRule, value: f64, now: u64) -> Result<bool> {
        // Check if condition is met
        let condition_met = match rule.condition.operator {
            ComparisonOperator::GreaterThan => value > rule.condition.threshold,
            ComparisonOperator::LessThan => value < rule.condition.threshold,
            ComparisonOperator::EqualTo => (value - rule.condition.threshold).abs() < 0.001,
            ComparisonOperator::NotEqualTo => (value - rule.condition.threshold).abs() >= 0.001,
            ComparisonOperator::GreaterThanOrEqual => value >= rule.condition.threshold,
            ComparisonOperator::LessThanOrEqual => value <= rule.condition.threshold,
        };

        if !condition_met {
            return Ok(false);
        }

        // Check cooldown period
        if self.is_in_cooldown(&rule.rule_id, now).await? {
            return Ok(false);
        }

        Ok(true)
    }

    async fn is_in_cooldown(&self, rule_id: &str, now: u64) -> Result<bool> {
        let active_alerts = self.active_alerts.read().await;
        
        if let Some(alert) = active_alerts.get(rule_id) {
            if alert.status == AlertStatus::Firing {
                let time_since_trigger = now - alert.triggered_at;
                
                // Get the rule's cooldown period
                let rules = self.rules.read().await;
                if let Some(rule) = rules.iter().find(|r| r.rule_id == rule_id) {
                    return Ok(time_since_trigger < rule.cooldown_seconds);
                }
            }
        }

        Ok(false)
    }

    async fn create_alert(&self, rule: &AlertRule, value: f64) -> Result<Alert> {
        let alert_id = Uuid::now_v7().to_string();
        let now = current_timestamp();

        let alert = Alert {
            alert_id: alert_id.clone(),
            rule_id: rule.rule_id.clone(),
            triggered_at: now,
            resolved_at: None,
            status: AlertStatus::Firing,
            value,
            threshold: rule.condition.threshold,
            message: format!("Alert '{}' triggered: value {:.2} {}", rule.name, value, self.format_operator(&rule.condition.operator)),
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("metric_name".to_string(), rule.metric_name.clone());
                ctx.insert("severity".to_string(), format!("{:?}", rule.severity));
                ctx
            },
        };

        // Store in database
        sqlx::raw_sql(&format!(
            "INSERT INTO alerts (alert_id, rule_id, triggered_at, status, value, threshold, message, context) \
             VALUES ('{}', '{}', {}, '{}', {}, {}, '{}', '{}')",
            alert.alert_id,
            alert.rule_id,
            alert.triggered_at,
            format!("{:?}", alert.status),
            alert.value,
            alert.threshold,
            escape_sql_string(&alert.message),
            serde_json::to_string(&alert.context)?
        ))
        .execute(&self.db_pool)
        .await?;

        // Add to active alerts
        self.active_alerts.write().await.insert(rule.rule_id.clone(), alert.clone());

        // Send notifications
        self.send_notifications(rule, &alert).await?;

        Ok(alert)
    }

    async fn send_notifications(&self, rule: &AlertRule, alert: &Alert) -> Result<()> {
        for channel in &rule.notification_channels {
            if !channel.enabled {
                continue;
            }

            match channel.channel_type {
                ChannelType::Email => {
                    tracing::info!("Would send email to {}: {}", channel.destination, alert.message);
                    // TODO: Implement email sending
                }
                ChannelType::Slack => {
                    tracing::info!("Would send Slack message to {}: {}", channel.destination, alert.message);
                    // TODO: Implement Slack integration
                }
                ChannelType::Webhook => {
                    self.send_webhook_notification(channel, alert).await?;
                }
                ChannelType::PagerDuty => {
                    tracing::info!("Would send PagerDuty alert to {}: {}", channel.destination, alert.message);
                    // TODO: Implement PagerDuty integration
                }
            }
        }

        Ok(())
    }

    async fn send_webhook_notification(&self, channel: &crate::types::NotificationChannel, alert: &Alert) -> Result<()> {
        let client = reqwest::Client::new();
        
        let payload = serde_json::json!({
            "alert_id": alert.alert_id,
            "rule_id": alert.rule_id,
            "message": alert.message,
            "severity": format!("{:?}", alert.status),
            "value": alert.value,
            "threshold": alert.threshold,
            "triggered_at": alert.triggered_at,
            "context": alert.context,
        });

        let response = client
            .post(&channel.destination)
            .json(&payload)
            .header("Content-Type", "application/json")
            .send()
            .await?;

        if response.status().is_success() {
            tracing::info!("Webhook notification sent successfully to {}", channel.destination);
        } else {
            tracing::warn!("Webhook notification failed: {}", response.status());
        }

        Ok(())
    }

    pub async fn resolve_alert(&self, alert_id: &str) -> Result<()> {
        let now = current_timestamp();

        sqlx::raw_sql(&format!(
            "UPDATE alerts SET status = 'Resolved', resolved_at = {} WHERE alert_id = '{}'",
            now, alert_id
        ))
        .execute(&self.db_pool)
        .await?;

        // Remove from active alerts
        let alert = sqlx::raw_sql(&format!(
            "SELECT rule_id FROM alerts WHERE alert_id = '{}'",
            alert_id
        ))
        .fetch_one(&self.db_pool)
        .await?;

        let rule_id: String = alert.try_get("rule_id")?;
        self.active_alerts.write().await.remove(&rule_id);

        Ok(())
    }
}
```

### Task 5: Report Generation (Day 8-10)

#### 5.1 Report Generator (report_generator.rs)
```rust
// src/report_generator.rs
use crate::types::{AggregatedMetric, TimeRange, AggregationType};
use crate::aggregator::MetricsAggregator;
use sqlx::PgPool;
use uuid::Uuid;
use anyhow::Result;
use chrono::{DateTime, Utc};

pub struct ReportGenerator {
    db_pool: PgPool,
    aggregator: MetricsAggregator,
}

impl ReportGenerator {
    pub fn new(db_pool: PgPool) -> Self {
        let aggregator = MetricsAggregator::new(db_pool.clone());
        
        ReportGenerator {
            db_pool,
            aggregator,
        }
    }

    pub async fn generate_statistics(
        &self,
        time_range: TimeRange,
    ) -> Result<StatisticsReport> {
        // Detection statistics
        let detection_stats = self.get_detection_statistics(time_range.clone()).await?;
        
        // Ban statistics
        let ban_stats = self.get_ban_statistics(time_range.clone()).await?;
        
        // Performance statistics
        let perf_stats = self.get_performance_statistics(time_range.clone()).await?;

        Ok(StatisticsReport {
            generated_at: current_timestamp(),
            time_range,
            detection: detection_stats,
            bans: ban_stats,
            performance: perf_stats,
        })
    }

    async fn get_detection_statistics(&self, time_range: TimeRange) -> Result<DetectionStatistics> {
        let time_range_str = format!("{} AND {}", time_range.start, time_range.end);

        // Total detections
        let total_rows = sqlx::raw_sql(&format!(
            "SELECT COUNT(*) as count FROM detection_results WHERE created_at BETWEEN {}",
            time_range_str
        ))
        .fetch_one(&self.db_pool)
        .await?;
        
        let total_detections: i64 = total_rows.try_get("count").unwrap_or(0);

        // By threat level
        let threat_rows = sqlx::raw_sql(&format!(
            "SELECT threat_level, COUNT(*) as count \
             FROM detection_results \
             WHERE created_at BETWEEN {} \
             GROUP BY threat_level",
            time_range_str
        ))
        .fetch_all(&self.db_pool)
        .await?;

        let mut by_threat_level = HashMap::new();
        for row in threat_rows {
            let level: String = row.try_get("threat_level")?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            by_threat_level.insert(level, count);
        }

        // Average risk score
        let avg_rows = sqlx::raw_sql(&format!(
            "SELECT AVG(risk_score) as avg_score \
             FROM detection_results \
             WHERE created_at BETWEEN {}",
            time_range_str
        ))
        .fetch_optional(&self.db_pool)
        .await?;

        let average_risk_score = avg_rows
            .and_then(|row| row.try_get::<f64, _>("avg_score").ok())
            .unwrap_or(0.0);

        Ok(DetectionStatistics {
            total_detections,
            by_threat_level,
            average_risk_score,
        })
    }

    async fn get_ban_statistics(&self, time_range: TimeRange) -> Result<BanStatistics> {
        let time_range_str = format!("{} AND {}", time_range.start, time_range.end);

        // Total bans
        let total_rows = sqlx::raw_sql(&format!(
            "SELECT COUNT(*) as count FROM bans WHERE created_at BETWEEN {}",
            time_range_str
        ))
        .fetch_one(&self.db_pool)
        .await?;
        
        let total_bans: i64 = total_rows.try_get("count").unwrap_or(0);

        // By ban type
        let type_rows = sqlx::raw_sql(&format!(
            "SELECT ban_type, COUNT(*) as count \
             FROM bans \
             WHERE created_at BETWEEN {} \
             GROUP BY ban_type",
            time_range_str
        ))
        .fetch_all(&self.db_pool)
        .await?;

        let mut by_type = HashMap::new();
        for row in type_rows {
            let ban_type: String = row.try_get("ban_type")?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            by_type.insert(ban_type, count);
        }

        // Appeal statistics
        let appeal_rows = sqlx::raw_sql(&format!(
            "SELECT status, COUNT(*) as count \
             FROM ban_appeals \
             WHERE submitted_at BETWEEN {} \
             GROUP BY status",
            time_range_str
        ))
        .fetch_all(&self.db_pool)
        .await?;

        let mut appeals = HashMap::new();
        for row in appeal_rows {
            let status: String = row.try_get("status")?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            appeals.insert(status, count);
        }

        Ok(BanStatistics {
            total_bans,
            by_type,
            appeals,
        })
    }

    async fn get_performance_statistics(&self, time_range: TimeRange) -> Result<PerformanceStatistics> {
        // Query performance metrics
        let query_latency = self.aggregator.aggregate_metrics(
            "query_latency_ms",
            AggregationType::Average,
            time_range.clone(),
        ).await?;

        let cache_hit_rate = self.aggregator.aggregate_metrics(
            "cache_hit_rate",
            AggregationType::Average,
            time_range.clone(),
        ).await?;

        let request_throughput = self.aggregator.aggregate_metrics(
            "requests_per_second",
            AggregationType::Sum,
            time_range.clone(),
        ).await?;

        Ok(PerformanceStatistics {
            average_query_latency_ms: query_latency.value,
            cache_hit_rate_percentage: cache_hit_rate.value * 100.0,
            total_requests: request_throughput.count,
        })
    }

    pub async fn export_report_csv(&self, report: &StatisticsReport) -> Result<String> {
        let mut csv = String::new();
        
        // Header
        csv.push_str("Category,Metric,Value\n");
        
        // Detection statistics
        csv.push_str(&format!("Detection,Total Detections,{}\n", report.detection.total_detections));
        csv.push_str(&format!("Detection,Average Risk Score,{:.2}\n", report.detection.average_risk_score));
        
        for (level, count) in &report.detection.by_threat_level {
            csv.push_str(&format!("Detection,{} Threat,{}\n", level, count));
        }
        
        // Ban statistics
        csv.push_str(&format!("Bans,Total Bans,{}\n", report.bans.total_bans));
        
        for (ban_type, count) in &report.bans.by_type {
            csv.push_str(&format!("Bans,{},{}\n", ban_type, count));
        }
        
        for (status, count) in &report.bans.appeals {
            csv.push_str(&format!("Appeals,{},{}\n", status, count));
        }
        
        // Performance statistics
        csv.push_str(&format!("Performance,Average Query Latency (ms),{:.2}\n", report.performance.average_query_latency_ms));
        csv.push_str(&format!("Performance,Cache Hit Rate (%),{:.2}\n", report.performance.cache_hit_rate_percentage));
        csv.push_str(&format!("Performance,Total Requests,{}\n", report.performance.total_requests));

        Ok(csv)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatisticsReport {
    pub generated_at: u64,
    pub time_range: TimeRange,
    pub detection: DetectionStatistics,
    pub bans: BanStatistics,
    pub performance: PerformanceStatistics,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DetectionStatistics {
    pub total_detections: i64,
    pub by_threat_level: HashMap<String, i64>,
    pub average_risk_score: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BanStatistics {
    pub total_bans: i64,
    pub by_type: HashMap<String, i64>,
    pub appeals: HashMap<String, i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceStatistics {
    pub average_query_latency_ms: f64,
    pub cache_hit_rate_percentage: f64,
    pub total_requests: i64,
}
```

### Task 6: Dashboard Data API (Day 10-12)

#### 6.1 Dashboard Service (dashboard.rs)
```rust
// src/dashboard.rs
use crate::types::{TimeRange};
use crate::report_generator::{ReportGenerator, StatisticsReport};
use sqlx::PgPool;
use anyhow::Result;

pub struct DashboardService {
    db_pool: PgPool,
    report_generator: ReportGenerator,
}

impl DashboardService {
    pub fn new(db_pool: PgPool) -> Self {
        let report_generator = ReportGenerator::new(db_pool.clone());
        
        DashboardService {
            db_pool,
            report_generator,
        }
    }

    pub async fn get_dashboard_data(&self) -> Result<DashboardData> {
        let now = current_timestamp();
        let last_hour = TimeRange {
            start: now - 3600,
            end: now,
        };
        let last_24h = TimeRange {
            start: now - 86400,
            end: now,
        };
        let last_7d = TimeRange {
            start: now - 604800,
            end: now,
        };

        // Get statistics for different time ranges
        let hour_stats = self.report_generator.generate_statistics(last_hour).await?;
        let day_stats = self.report_generator.generate_statistics(last_24h).await?;
        let week_stats = self.report_generator.generate_statistics(last_7d).await?;

        // Get active alerts
        let active_alerts = self.get_active_alerts().await?;

        // Get system health
        let system_health = self.get_system_health().await?;

        Ok(DashboardData {
            last_hour: hour_stats,
            last_24h: day_stats,
            last_7d: week_stats,
            active_alerts,
            system_health,
        })
    }

    async fn get_active_alerts(&self) -> Result<Vec<AlertSummary>> {
        let rows = sqlx::raw_sql(
            "SELECT alert_id, alert.rule_id, alert_rules.name as rule_name, alert.message, alert.triggered_at, alert_rules.severity \
             FROM alerts \
             JOIN alert_rules ON alerts.rule_id = alert_rules.rule_id \
             WHERE alerts.status = 'Firing' \
             ORDER BY alert.triggered_at DESC \
             LIMIT 10"
        )
        .fetch_all(&self.db_pool)
        .await?;

        rows.into_iter()
            .map(|row| -> Result<AlertSummary> {
                Ok(AlertSummary {
                    alert_id: row.try_get("alert_id")?,
                    rule_id: row.try_get("rule_id")?,
                    rule_name: row.try_get("rule_name")?,
                    message: row.try_get("message")?,
                    triggered_at: row.try_get("triggered_at")?,
                    severity: row.try_get::<String, _>("severity")?,
                })
            })
            .collect()
    }

    async fn get_system_health(&self) -> Result<SystemHealth> {
        // Database health
        let db_health = self.check_database_health().await?;
        
        // Redis health (if configured)
        let redis_health = self.check_redis_health().await.ok();
        
        // Recent error rate
        let error_rate = self.get_recent_error_rate().await?;

        Ok(SystemHealth {
            database: db_health,
            redis: redis_health,
            error_rate_percentage: error_rate,
            status: if db_health && error_rate < 5.0 {
                "Healthy".to_string()
            } else {
                "Degraded".to_string()
            },
        })
    }

    async fn check_database_health(&self) -> Result<bool> {
        match sqlx::raw_sql("SELECT 1")
            .fetch_one(&self.db_pool)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn check_redis_health(&self) -> Result<bool> {
        // TODO: Implement Redis health check
        Ok(true)
    }

    async fn get_recent_error_rate(&self) -> Result<f64> {
        let now = current_timestamp();
        let five_minutes_ago = now - 300;

        let error_rows = sqlx::raw_sql(&format!(
            "SELECT COUNT(*) as count FROM metrics \
             WHERE metric_name = 'http_errors' \
             AND timestamp BETWEEN {} AND {}",
            five_minutes_ago, now
        ))
        .fetch_optional(&self.db_pool)
        .await?;

        let total_rows = sqlx::raw_sql(&format!(
            "SELECT COUNT(*) as count FROM metrics \
             WHERE metric_name = 'http_requests' \
             AND timestamp BETWEEN {} AND {}",
            five_minutes_ago, now
        ))
        .fetch_optional(&self.db_pool)
        .await?;

        let errors: i64 = error_rows.and_then(|r| r.try_get("count").ok()).unwrap_or(0);
        let total: i64 = total_rows.and_then(|r| r.try_get("count").ok()).unwrap_or(1);

        Ok((errors as f64 / total as f64) * 100.0)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DashboardData {
    pub last_hour: StatisticsReport,
    pub last_24h: StatisticsReport,
    pub last_7d: StatisticsReport,
    pub active_alerts: Vec<AlertSummary>,
    pub system_health: SystemHealth,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertSummary {
    pub alert_id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub message: String,
    pub triggered_at: u64,
    pub severity: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemHealth {
    pub database: bool,
    pub redis: Option<bool>,
    pub error_rate_percentage: f64,
    pub status: String,
}
```

### Task 7: API Integration (Day 12-14)

#### 7.1 Axum Endpoints (api.rs)
```rust
// src/api.rs
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::types::TimeRange;

#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    pub start: Option<u64>,
    pub end: Option<u64>,
}

pub async fn get_statistics(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
) -> Result<Json<StatisticsReport>, ApiError> {
    let time_range = TimeRange {
        start: query.start.unwrap_or(current_timestamp() - 86400),
        end: query.end.unwrap_or(current_timestamp()),
    };

    let report = state.report_generator.generate_statistics(time_range).await?;
    Ok(Json(report))
}

pub async fn get_dashboard(
    State(state): State<AppState>,
) -> Result<Json<DashboardData>, ApiError> {
    let data = state.dashboard_service.get_dashboard_data().await?;
    Ok(Json(data))
}

#[derive(Debug, Deserialize)]
pub struct CreateAlertRuleRequest {
    pub name: String,
    pub description: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub notification_channels: Vec<NotificationChannel>,
    pub cooldown_seconds: u64,
}

pub async fn create_alert_rule(
    State(state): State<AppState>,
    Json(req): Json<CreateAlertRuleRequest>,
    admin_id: String,
) -> Result<Json<AlertRule>, ApiError> {
    let rule_id = Uuid::now_v7().to_string();
    let now = current_timestamp();

    let rule = AlertRule {
        rule_id: rule_id.clone(),
        name: req.name,
        description: req.description,
        metric_name: req.metric_name,
        condition: req.condition,
        severity: req.severity,
        enabled: true,
        notification_channels: req.notification_channels,
        cooldown_seconds: req.cooldown_seconds,
        created_at: now,
    };

    sqlx::raw_sql(&format!(
        "INSERT INTO alert_rules (rule_id, name, description, metric_name, condition, severity, enabled, notification_channels, cooldown_seconds, created_at) \
         VALUES ('{}', '{}', '{}', '{}', '{}', '{}', true, '{}', {}, {})",
        rule.rule_id,
        rule.name,
        escape_sql_string(&rule.description),
        rule.metric_name,
        serde_json::to_string(&rule.condition)?,
        format!("{:?}", rule.severity),
        serde_json::to_string(&rule.notification_channels)?,
        rule.cooldown_seconds,
        rule.created_at
    ))
    .execute(&state.db_pool)
    .await?;

    // Reload rules
    state.alert_manager.load_rules().await?;

    Ok(Json(rule))
}

pub async fn get_alerts(
    State(state): State<AppState>,
    Query(filter): Query<AlertFilter>,
) -> Result<Json<Vec<Alert>>, ApiError> {
    let mut query = String::from("SELECT alert_id, rule_id, triggered_at, resolved_at, status, value, threshold, message, context FROM alerts WHERE 1=1");

    if let Some(status) = filter.status {
        query.push_str(&format!(" AND status = '{}'", status));
    }

    if let Some(rule_id) = filter.rule_id {
        query.push_str(&format!(" AND rule_id = '{}'", rule_id));
    }

    query.push_str(" ORDER BY triggered_at DESC LIMIT 100");

    let rows = sqlx::raw_sql(&query)
        .fetch_all(&state.db_pool)
        .await?;

    let alerts: Vec<Alert> = rows.into_iter()
        .map(|row| -> Result<Alert> {
            Ok(Alert {
                alert_id: row.try_get("alert_id")?,
                rule_id: row.try_get("rule_id")?,
                triggered_at: row.try_get("triggered_at")?,
                resolved_at: {
                    let resolved: Option<i64> = row.try_get("resolved_at").ok();
                    resolved.map(|r| r as u64)
                },
                status: match row.try_get::<String, _>("status")?.as_str() {
                    "Firing" => AlertStatus::Firing,
                    "Resolved" => AlertStatus::Resolved,
                    "Acknowledged" => AlertStatus::Acknowledged,
                    _ => return Err(anyhow::anyhow!("Invalid alert status")),
                },
                value: row.try_get("value")?,
                threshold: row.try_get("threshold")?,
                message: row.try_get("message")?,
                context: serde_json::from_str(row.try_get("context")?).unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Json(alerts))
}

#[derive(Debug, Deserialize)]
pub struct AlertFilter {
    pub status: Option<String>,
    pub rule_id: Option<String>,
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collection() {
        let pool = create_test_pool().await;
        let collector = MetricsCollector::new(pool, "redis://localhost", 100).unwrap();
        
        collector.record_gauge("test_metric", 42.0, HashMap::new()).await.unwrap();
        
        // Verify metric was recorded
        // (Would need to query database)
    }

    #[tokio::test]
    async fn test_aggregation() {
        let pool = create_test_pool().await;
        let aggregator = MetricsAggregator::new(pool);
        
        let time_range = TimeRange {
            start: 1000,
            end: 2000,
        };
        
        // Test various aggregation types
    }

    #[tokio::test]
    async fn test_alert_triggering() {
        let pool = create_test_pool().await;
        let alert_manager = AlertManager::new(pool, "redis://localhost").unwrap();
        
        // Create test rule
        // Trigger alert
        // Verify alert was created
    }

    #[tokio::test]
    async fn test_report_generation() {
        let pool = create_test_pool().await;
        let generator = ReportGenerator::new(pool);
        
        let time_range = TimeRange {
            start: current_timestamp() - 3600,
            end: current_timestamp(),
        };
        
        let report = generator.generate_statistics(time_range).await.unwrap();
        
        assert!(!report.detection.total_detections < 0);
    }
}
```

### Integration Tests
- End-to-end metrics collection and aggregation
- Alert rule creation and triggering
- Report generation and export
- Dashboard data retrieval
- Alert notification delivery

## Performance Requirements

- **Metric Ingestion**: > 100,000 metrics/second
- **Aggregation Latency**: < 5 seconds
- **Query Response Time**: < 100ms for 24h range
- **Alert Evaluation**: < 10ms per rule
- **Report Generation**: < 5 seconds for 7d range

## Security Considerations

### Data Access
- Role-based access control for dashboard
- Audit logging for all administrative actions
- Secure API endpoints with authentication

### Data Retention
- Automatic cleanup of old metrics
- Configurable retention policies
- GDPR compliance for sensitive data

## Dependencies

- `sqlx` - Database (use raw_sql)
- `redis` - Real-time metrics pub/sub
- `axum` - Web framework
- `tokio` - Async runtime
- `serde` - Serialization
- `chrono` - Date/time handling
- `reqwest` - HTTP client for webhooks
- `anyhow` - Error handling

## Deliverables

1. ✅ Metrics collection service
2. ✅ Metrics aggregation engine
3. ✅ Alerting system
4. ✅ Report generator
5. ✅ Dashboard API
6. ✅ Alert notification delivery
7. ✅ Unit and integration tests
8. ✅ Documentation

## Next Steps

After completing this phase, proceed to:
- **008g**: Admin Dashboard UI
- **Integration testing** with all components
- **Deployment** to production

## Notes

- Follow project coding style: snake_case, match over if, early returns
- Use `sqlx::raw_sql` for all database queries (per project guidelines)
- All timestamps use Unix timestamps (u64)
- Use `Uuid::now_v7()` for all ID generation
- Implement graceful degradation if external services fail
- Cache aggregated metrics to reduce database load
- Alert notifications should be idempotent
- Dashboard queries should be optimized with proper indexes