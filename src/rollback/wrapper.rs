//! Safe change wrapper for configuration modifications.
//!
//! This module provides a safe execution environment for configuration changes:
//! - Automatic snapshot creation before changes
//! - Change journaling with full audit trail
//! - Pre/post change hooks for extensibility
//! - Automatic rollback on failure
//! - Confirmation mechanism for critical changes
//!
//! # Architecture
//!
//! - [`SafeChangeExecutor`] - Core executor wrapping all change operations
//! - [`ChangeHook`] - Trait for pre/post execution hooks
//! - [`SafeToolWrapper`] - Wrapper for Tool trait implementations
//! - [`RollbackEngine`] - Re-exported from engine module for rollback handling

use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use super::engine::RollbackEngine;
use super::journal::{ChangeRecord, ChangeStatus, ChangeType, ChangeJournal};
use super::snapshot::{ConfigSnapshot, CreateSnapshotOptions, SnapshotManager};
use crate::tools::traits::{Tool, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub auto_snapshot: bool,
    pub auto_rollback_on_failure: bool,
    pub require_confirmation: bool,
    pub confirmation_timeout_secs: u64,
    pub max_retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: true,
            auto_rollback_on_failure: true,
            require_confirmation: false,
            confirmation_timeout_secs: 30,
            max_retry_attempts: 3,
            retry_delay_ms: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeContext {
    pub operator: String,
    pub change_type: ChangeType,
    pub target: String,
    pub description: String,
    pub metadata: HashMap<String, String>,
}

impl ChangeContext {
    pub fn new(operator: &str, change_type: ChangeType, target: &str, description: &str) -> Self {
        Self {
            operator: operator.to_string(),
            change_type,
            target: target.to_string(),
            description: description.to_string(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResult {
    pub success: bool,
    pub change_id: String,
    pub snapshot_id: Option<String>,
    pub output: String,
    pub error: Option<String>,
    pub requires_confirmation: bool,
    pub rolled_back: bool,
    pub duration_ms: u64,
}

impl ChangeResult {
    pub fn success(change_id: &str, snapshot_id: Option<&str>, output: &str, duration_ms: u64) -> Self {
        Self {
            success: true,
            change_id: change_id.to_string(),
            snapshot_id: snapshot_id.map(|s| s.to_string()),
            output: output.to_string(),
            error: None,
            requires_confirmation: false,
            rolled_back: false,
            duration_ms,
        }
    }

    pub fn failure(change_id: &str, snapshot_id: Option<&str>, error: &str, rolled_back: bool, duration_ms: u64) -> Self {
        Self {
            success: false,
            change_id: change_id.to_string(),
            snapshot_id: snapshot_id.map(|s| s.to_string()),
            output: String::new(),
            error: Some(error.to_string()),
            requires_confirmation: false,
            rolled_back,
            duration_ms,
        }
    }

    pub fn pending_confirmation(change_id: &str, snapshot_id: Option<&str>, output: &str) -> Self {
        Self {
            success: true,
            change_id: change_id.to_string(),
            snapshot_id: snapshot_id.map(|s| s.to_string()),
            output: output.to_string(),
            error: None,
            requires_confirmation: true,
            rolled_back: false,
            duration_ms: 0,
        }
    }
}

#[async_trait]
pub trait ChangeHook: Send + Sync {
    fn name(&self) -> &str;

    async fn pre_execute(&self, ctx: &ChangeContext) -> Result<HookDecision>;

    async fn post_execute(&self, ctx: &ChangeContext, result: &ChangeResult) -> Result<()>;

    async fn on_failure(&self, ctx: &ChangeContext, error: &str) -> Result<()>;

    async fn on_rollback(&self, ctx: &ChangeContext, reason: &str) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    Continue,
    Abort(String),
}

pub struct LoggingHook;

impl LoggingHook {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoggingHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChangeHook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }

    async fn pre_execute(&self, ctx: &ChangeContext) -> Result<HookDecision> {
        tracing::info!(
            operator = %ctx.operator,
            change_type = ?ctx.change_type,
            target = %ctx.target,
            description = %ctx.description,
            "Change execution starting"
        );
        Ok(HookDecision::Continue)
    }

    async fn post_execute(&self, ctx: &ChangeContext, result: &ChangeResult) -> Result<()> {
        tracing::info!(
            operator = %ctx.operator,
            target = %ctx.target,
            success = result.success,
            change_id = %result.change_id,
            duration_ms = result.duration_ms,
            "Change execution completed"
        );
        Ok(())
    }

    async fn on_failure(&self, ctx: &ChangeContext, error: &str) -> Result<()> {
        tracing::error!(
            operator = %ctx.operator,
            target = %ctx.target,
            error = %error,
            "Change execution failed"
        );
        Ok(())
    }

    async fn on_rollback(&self, ctx: &ChangeContext, reason: &str) -> Result<()> {
        tracing::warn!(
            operator = %ctx.operator,
            target = %ctx.target,
            reason = %reason,
            "Change rolled back"
        );
        Ok(())
    }
}

pub struct ValidationHook {
    validators: Vec<Box<dyn Fn(&ChangeContext) -> Result<()> + Send + Sync>>,
}

impl ValidationHook {
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    pub fn with_validator<F: Fn(&ChangeContext) -> Result<()> + Send + Sync + 'static>(mut self, validator: F) -> Self {
        self.validators.push(Box::new(validator));
        self
    }
}

impl Default for ValidationHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChangeHook for ValidationHook {
    fn name(&self) -> &str {
        "validation"
    }

    async fn pre_execute(&self, ctx: &ChangeContext) -> Result<HookDecision> {
        for validator in &self.validators {
            validator(ctx)?;
        }
        Ok(HookDecision::Continue)
    }

    async fn post_execute(&self, _ctx: &ChangeContext, _result: &ChangeResult) -> Result<()> {
        Ok(())
    }

    async fn on_failure(&self, _ctx: &ChangeContext, _error: &str) -> Result<()> {
        Ok(())
    }

    async fn on_rollback(&self, _ctx: &ChangeContext, _reason: &str) -> Result<()> {
        Ok(())
    }
}

pub struct NotificationHook {
    notify_url: Option<String>,
}

impl NotificationHook {
    pub fn new() -> Self {
        Self { notify_url: None }
    }

    pub fn with_url(mut self, url: &str) -> Self {
        self.notify_url = Some(url.to_string());
        self
    }
}

impl Default for NotificationHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChangeHook for NotificationHook {
    fn name(&self) -> &str {
        "notification"
    }

    async fn pre_execute(&self, ctx: &ChangeContext) -> Result<HookDecision> {
        if let Some(url) = &self.notify_url {
            tracing::debug!(
                url = %url,
                operator = %ctx.operator,
                target = %ctx.target,
                "Would send pre-execution notification"
            );
        }
        Ok(HookDecision::Continue)
    }

    async fn post_execute(&self, ctx: &ChangeContext, result: &ChangeResult) -> Result<()> {
        if let Some(url) = &self.notify_url {
            tracing::debug!(
                url = %url,
                operator = %ctx.operator,
                target = %ctx.target,
                success = result.success,
                "Would send post-execution notification"
            );
        }
        Ok(())
    }

    async fn on_failure(&self, ctx: &ChangeContext, error: &str) -> Result<()> {
        if let Some(url) = &self.notify_url {
            tracing::debug!(
                url = %url,
                operator = %ctx.operator,
                target = %ctx.target,
                error = %error,
                "Would send failure notification"
            );
        }
        Ok(())
    }

    async fn on_rollback(&self, ctx: &ChangeContext, reason: &str) -> Result<()> {
        if let Some(url) = &self.notify_url {
            tracing::debug!(
                url = %url,
                operator = %ctx.operator,
                target = %ctx.target,
                reason = %reason,
                "Would send rollback notification"
            );
        }
        Ok(())
    }
}

pub struct SafeChangeExecutor {
    snapshot_manager: SnapshotManager,
    journal: ChangeJournal,
    rollback_engine: Arc<RollbackEngine>,
    config: ExecutorConfig,
    hooks: Arc<RwLock<Vec<Arc<dyn ChangeHook>>>>,
    pending_confirmations: Arc<AsyncMutex<HashMap<String, PendingConfirmation>>>,
}

#[derive(Debug, Clone)]
struct PendingConfirmation {
    change_id: String,
    snapshot_id: Option<String>,
    context: ChangeContext,
    created_at: chrono::DateTime<Utc>,
}

impl SafeChangeExecutor {
    pub fn new(
        snapshot_manager: SnapshotManager,
        journal: ChangeJournal,
        rollback_engine: Arc<RollbackEngine>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            snapshot_manager,
            journal,
            rollback_engine,
            config,
            hooks: Arc::new(RwLock::new(Vec::new())),
            pending_confirmations: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    pub fn add_hook(&self, hook: Arc<dyn ChangeHook>) {
        self.hooks.write().push(hook);
    }

    pub fn remove_hook(&self, name: &str) -> bool {
        let mut hooks = self.hooks.write();
        if let Some(pos) = hooks.iter().position(|h| h.name() == name) {
            hooks.remove(pos);
            true
        } else {
            false
        }
    }

    pub async fn execute<F, Fut>(
        &self,
        ctx: ChangeContext,
        operation: F,
    ) -> Result<ChangeResult>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<String>> + Send,
    {
        let start = std::time::Instant::now();
        let change_id = Uuid::new_v4().to_string();

        let hooks: Vec<Arc<dyn ChangeHook>> = self.hooks.read().iter().cloned().collect();
        for hook in &hooks {
            match hook.pre_execute(&ctx).await {
                Ok(HookDecision::Continue) => {}
                Ok(HookDecision::Abort(reason)) => {
                    return Ok(ChangeResult::failure(
                        &change_id,
                        None,
                        &reason,
                        false,
                        start.elapsed().as_millis() as u64,
                    ));
                }
                Err(e) => {
                    return Ok(ChangeResult::failure(
                        &change_id,
                        None,
                        &format!("Pre-execute hook failed: {}", e),
                        false,
                        start.elapsed().as_millis() as u64,
                    ));
                }
            }
        }

        let snapshot_id = if self.config.auto_snapshot {
            match self.create_pre_change_snapshot(&ctx).await {
                Ok(snapshot) => Some(snapshot.id),
                Err(e) => {
                    tracing::warn!("Failed to create pre-change snapshot: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let record = ChangeRecord::new(
            &ctx.operator,
            ctx.change_type,
            &ctx.target,
            None,
            None,
            snapshot_id.clone(),
        );
        let record_id = record.id.clone();
        self.journal.record(&record)?;

        let result = match operation().await {
            Ok(output) => {
                self.journal.update_status(&record_id, ChangeStatus::Applied)?;
                
                let result = ChangeResult::success(
                    &record_id,
                    snapshot_id.as_deref(),
                    &output,
                    start.elapsed().as_millis() as u64,
                );

                for hook in &hooks {
                    if let Err(e) = hook.post_execute(&ctx, &result).await {
                        tracing::warn!("Post-execute hook '{}' failed: {}", hook.name(), e);
                    }
                }

                result
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.journal.update_status(&record_id, ChangeStatus::Failed)?;

                for hook in &hooks {
                    if let Err(he) = hook.on_failure(&ctx, &error_msg).await {
                        tracing::warn!("On-failure hook '{}' failed: {}", hook.name(), he);
                    }
                }

                let rolled_back = if self.config.auto_rollback_on_failure {
                    if let Some(ref sid) = snapshot_id {
                        match self.rollback_engine.rollback_to_snapshot(sid).await {
                            Ok(_) => {
                                self.journal.set_rollback_id(&record_id, &Uuid::new_v4().to_string())?;
                                
                                for hook in &hooks {
                                    if let Err(he) = hook.on_rollback(&ctx, &error_msg).await {
                                        tracing::warn!("On-rollback hook '{}' failed: {}", hook.name(), he);
                                    }
                                }
                                true
                            }
                            Err(re) => {
                                tracing::error!("Rollback failed: {}", re);
                                false
                            }
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                ChangeResult::failure(
                    &record_id,
                    snapshot_id.as_deref(),
                    &error_msg,
                    rolled_back,
                    start.elapsed().as_millis() as u64,
                )
            }
        };

        Ok(result)
    }

    async fn create_pre_change_snapshot(&self, ctx: &ChangeContext) -> Result<ConfigSnapshot> {
        let options = CreateSnapshotOptions {
            label: Some(format!("pre-change-{}", ctx.change_type)),
            description: Some(format!(
                "Auto snapshot before: {} by {}",
                ctx.description, ctx.operator
            )),
            config_dir: None,
            include_patterns: None,
        };

        self.snapshot_manager.create_snapshot(options)
    }

    pub async fn confirm_change(&self, change_id: &str) -> Result<bool> {
        let mut pending = self.pending_confirmations.lock().await;
        
        if let Some(confirmation) = pending.remove(change_id) {
            let elapsed = (Utc::now() - confirmation.created_at)
                .num_seconds();
            
            if elapsed as u64 > self.config.confirmation_timeout_secs {
                bail!("Confirmation timeout exceeded for change {}", change_id);
            }

            self.journal.update_status(change_id, ChangeStatus::Applied)?;
            
            tracing::info!(
                change_id = %change_id,
                operator = %confirmation.context.operator,
                "Change confirmed"
            );
            
            Ok(true)
        } else {
            bail!("No pending confirmation found for change {}", change_id);
        }
    }

    pub async fn cancel_change(&self, change_id: &str) -> Result<bool> {
        let mut pending = self.pending_confirmations.lock().await;
        
        if let Some(confirmation) = pending.remove(change_id) {
            if let Some(snapshot_id) = &confirmation.snapshot_id {
                self.rollback_engine.rollback_to_snapshot(snapshot_id).await?;
                self.journal.set_rollback_id(change_id, &Uuid::new_v4().to_string())?;
            }

            self.journal.update_status(change_id, ChangeStatus::RolledBack)?;
            
            tracing::info!(
                change_id = %change_id,
                operator = %confirmation.context.operator,
                "Change cancelled and rolled back"
            );
            
            Ok(true)
        } else {
            bail!("No pending confirmation found for change {}", change_id);
        }
    }

    pub fn get_journal(&self) -> &ChangeJournal {
        &self.journal
    }

    pub fn get_rollback_engine(&self) -> &Arc<RollbackEngine> {
        &self.rollback_engine
    }

    pub fn get_config(&self) -> &ExecutorConfig {
        &self.config
    }
}

pub struct SafeToolWrapper<T: Tool> {
    inner: T,
    executor: Arc<SafeChangeExecutor>,
    change_type: ChangeType,
    operator: String,
}

impl<T: Tool> SafeToolWrapper<T> {
    pub fn new(
        inner: T,
        executor: Arc<SafeChangeExecutor>,
        change_type: ChangeType,
        operator: &str,
    ) -> Self {
        Self {
            inner,
            executor,
            change_type,
            operator: operator.to_string(),
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T: Tool + 'static> Tool for SafeToolWrapper<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.inner.parameters_schema()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let ctx = ChangeContext::new(
            &self.operator,
            self.change_type,
            target,
            &format!("Tool execution: {}", self.inner.name()),
        );

        let args_clone = args.clone();
        let inner = &self.inner;
        
        let result = self.executor.execute(ctx, || async move {
            let tool_result = inner.execute(args_clone).await?;
            Ok(tool_result.output)
        }).await?;

        Ok(ToolResult {
            success: result.success,
            output: result.output,
            error: result.error,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeOperation {
    pub id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub change_type: ChangeType,
    pub target: String,
    pub description: String,
}

impl ChangeOperation {
    pub fn new(
        tool_name: &str,
        args: serde_json::Value,
        change_type: ChangeType,
        target: &str,
        description: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            args,
            change_type,
            target: target.to_string(),
            description: description.to_string(),
        }
    }
}

pub struct BatchChangeExecutor {
    executor: Arc<SafeChangeExecutor>,
}

impl BatchChangeExecutor {
    pub fn new(executor: Arc<SafeChangeExecutor>) -> Self {
        Self { executor }
    }

    pub async fn execute_batch(
        &self,
        operations: Vec<ChangeOperation>,
        tools: &HashMap<String, Arc<dyn Tool>>,
        operator: &str,
        stop_on_failure: bool,
    ) -> Result<Vec<ChangeResult>> {
        let mut results = Vec::with_capacity(operations.len());

        for op in operations {
            let tool = tools
                .get(&op.tool_name)
                .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", op.tool_name))?;

            let ctx = ChangeContext::new(
                operator,
                op.change_type,
                &op.target,
                &op.description,
            );

            let args = op.args.clone();
            let tool_clone = tool.clone();
            
            let result = self.executor.execute(ctx, || async move {
                let tool_result = tool_clone.execute(args).await?;
                Ok(tool_result.output)
            }).await?;

            let failed = !result.success;
            results.push(result);

            if stop_on_failure && failed {
                break;
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, TempDir, SafeChangeExecutor) {
        let zeroclaw_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();

        let config_path = config_dir.path();
        std::fs::write(config_path.join("network"), "config interface 'lan'\n").unwrap();
        std::fs::write(config_path.join("firewall"), "config defaults\n").unwrap();

        let snapshot_manager = SnapshotManager::new(zeroclaw_dir.path(), Some(config_path)).unwrap();
        let journal = ChangeJournal::open(zeroclaw_dir.path()).unwrap();
        let rollback_engine = Arc::new(
            RollbackEngine::new(zeroclaw_dir.path(), Some(config_path), None).unwrap()
        );
        let config = ExecutorConfig::default();
        
        let executor = SafeChangeExecutor::new(snapshot_manager, journal, rollback_engine, config);
        (zeroclaw_dir, config_dir, executor)
    }

    #[test]
    fn executor_config_default() {
        let config = ExecutorConfig::default();
        assert!(config.auto_snapshot);
        assert!(config.auto_rollback_on_failure);
        assert!(!config.require_confirmation);
        assert_eq!(config.confirmation_timeout_secs, 30);
    }

    #[test]
    fn change_context_builder() {
        let ctx = ChangeContext::new("zeroclaw_user", ChangeType::FirewallRule, "/etc/firewall", "Add rule")
            .with_metadata("key1", "value1");

        assert_eq!(ctx.operator, "zeroclaw_user");
        assert_eq!(ctx.change_type, ChangeType::FirewallRule);
        assert_eq!(ctx.target, "/etc/firewall");
        assert_eq!(ctx.description, "Add rule");
        assert_eq!(ctx.metadata.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn change_result_success() {
        let result = ChangeResult::success("change-123", Some("snap-456"), "Operation completed", 150);
        
        assert!(result.success);
        assert_eq!(result.change_id, "change-123");
        assert_eq!(result.snapshot_id, Some("snap-456".to_string()));
        assert_eq!(result.output, "Operation completed");
        assert!(result.error.is_none());
        assert!(!result.requires_confirmation);
        assert!(!result.rolled_back);
        assert_eq!(result.duration_ms, 150);
    }

    #[test]
    fn change_result_failure() {
        let result = ChangeResult::failure("change-123", Some("snap-456"), "Error occurred", true, 200);
        
        assert!(!result.success);
        assert_eq!(result.error, Some("Error occurred".to_string()));
        assert!(result.rolled_back);
    }

    #[test]
    fn change_result_pending_confirmation() {
        let result = ChangeResult::pending_confirmation("change-123", Some("snap-456"), "Waiting");
        
        assert!(result.success);
        assert!(result.requires_confirmation);
    }

    #[tokio::test]
    async fn logging_hook_pre_execute() {
        let hook = LoggingHook::new();
        let ctx = ChangeContext::new("zeroclaw_user", ChangeType::FirewallRule, "/etc/firewall", "Test");
        
        let decision = hook.pre_execute(&ctx).await.unwrap();
        assert_eq!(decision, HookDecision::Continue);
    }

    #[tokio::test]
    async fn validation_hook_with_validator() {
        let hook = ValidationHook::new()
            .with_validator(|ctx| {
                if ctx.operator.is_empty() {
                    bail!("Operator cannot be empty");
                }
                Ok(())
            });

        let ctx_valid = ChangeContext::new("zeroclaw_user", ChangeType::FirewallRule, "/etc/firewall", "Test");
        let ctx_invalid = ChangeContext::new("", ChangeType::FirewallRule, "/etc/firewall", "Test");

        let decision = hook.pre_execute(&ctx_valid).await.unwrap();
        assert_eq!(decision, HookDecision::Continue);

        let result = hook.pre_execute(&ctx_invalid).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn safe_change_executor_successful_operation() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();

        let ctx = ChangeContext::new(
            "zeroclaw_user",
            ChangeType::FirewallRule,
            "/etc/firewall/user.rules",
            "Add firewall rule",
        );

        let result = executor.execute(ctx, || async {
            Ok("Rule added successfully".to_string())
        }).await.unwrap();

        assert!(result.success);
        assert!(result.snapshot_id.is_some());
        assert_eq!(result.output, "Rule added successfully");
        assert!(!result.rolled_back);
    }

    #[tokio::test]
    async fn safe_change_executor_failed_operation_with_rollback() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();

        let ctx = ChangeContext::new(
            "zeroclaw_user",
            ChangeType::NetworkConfig,
            "/etc/network/interfaces",
            "Modify network config",
        );

        let result = executor.execute(ctx, || async {
            bail!("Network configuration failed");
        }).await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.rolled_back);
    }

    #[tokio::test]
    async fn safe_change_executor_with_hooks() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();
        
        executor.add_hook(Arc::new(LoggingHook::new()));
        executor.add_hook(Arc::new(ValidationHook::new()));

        let ctx = ChangeContext::new(
            "zeroclaw_user",
            ChangeType::ServiceConfig,
            "/etc/systemd/system/test.service",
            "Update service config",
        );

        let result = executor.execute(ctx, || async {
            Ok("Service updated".to_string())
        }).await.unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn hook_abort_prevents_execution() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();

        struct AbortHook;
        
        #[async_trait]
        impl ChangeHook for AbortHook {
            fn name(&self) -> &str { "abort" }
            
            async fn pre_execute(&self, _ctx: &ChangeContext) -> Result<HookDecision> {
                Ok(HookDecision::Abort("Operation aborted by hook".to_string()))
            }
            
            async fn post_execute(&self, _ctx: &ChangeContext, _result: &ChangeResult) -> Result<()> { Ok(()) }
            async fn on_failure(&self, _ctx: &ChangeContext, _error: &str) -> Result<()> { Ok(()) }
            async fn on_rollback(&self, _ctx: &ChangeContext, _reason: &str) -> Result<()> { Ok(()) }
        }

        executor.add_hook(Arc::new(AbortHook));

        let ctx = ChangeContext::new("zeroclaw_user", ChangeType::Other, "/test", "Test");
        
        let result = executor.execute(ctx, || async {
            Ok("This should not execute".to_string())
        }).await.unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("aborted"));
    }

    #[tokio::test]
    async fn safe_tool_wrapper_wraps_tool() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();
        
        struct TestTool;
        
        #[async_trait]
        impl Tool for TestTool {
            fn name(&self) -> &str { "test_tool" }
            fn description(&self) -> &str { "A test tool" }
            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target": { "type": "string" }
                    }
                })
            }
            async fn execute(&self, args: serde_json::Value) -> Result<ToolResult> {
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("default");
                Ok(ToolResult {
                    success: true,
                    output: format!("Executed on: {}", target),
                    error: None,
                })
            }
        }

        let tool = TestTool;
        let wrapper = SafeToolWrapper::new(
            tool,
            Arc::new(executor),
            ChangeType::Other,
            "zeroclaw_user",
        );

        assert_eq!(wrapper.name(), "test_tool");
        assert_eq!(wrapper.description(), "A test tool");

        let result = wrapper.execute(serde_json::json!({ "target": "/etc/test" })).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("/etc/test"));
    }

    #[test]
    fn change_operation_creation() {
        let op = ChangeOperation::new(
            "firewall_tool",
            serde_json::json!({ "rule": "allow 80" }),
            ChangeType::FirewallRule,
            "/etc/firewall/rules",
            "Add HTTP rule",
        );

        assert!(!op.id.is_empty());
        assert_eq!(op.tool_name, "firewall_tool");
        assert_eq!(op.change_type, ChangeType::FirewallRule);
        assert_eq!(op.target, "/etc/firewall/rules");
    }

    #[test]
    fn remove_hook_by_name() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();
        
        executor.add_hook(Arc::new(LoggingHook::new()));
        executor.add_hook(Arc::new(NotificationHook::new()));

        assert!(executor.remove_hook("logging"));
        assert!(!executor.remove_hook("nonexistent"));
    }

    #[tokio::test]
    async fn batch_executor_processes_operations() {
        let (_zeroclaw_dir, _config_dir, executor) = setup_test_env();
        
        struct SuccessTool;
        
        #[async_trait]
        impl Tool for SuccessTool {
            fn name(&self) -> &str { "success_tool" }
            fn description(&self) -> &str { "Always succeeds" }
            fn parameters_schema(&self) -> serde_json::Value { serde_json::json!({}) }
            async fn execute(&self, _args: serde_json::Value) -> Result<ToolResult> {
                Ok(ToolResult { success: true, output: "OK".to_string(), error: None })
            }
        }

        let batch = BatchChangeExecutor::new(Arc::new(executor));
        let tools: HashMap<String, Arc<dyn Tool>> = vec![
            ("success_tool".to_string(), Arc::new(SuccessTool) as Arc<dyn Tool>),
        ].into_iter().collect();

        let operations = vec![
            ChangeOperation::new("success_tool", serde_json::json!({}), ChangeType::Other, "/test1", "Op 1"),
            ChangeOperation::new("success_tool", serde_json::json!({}), ChangeType::Other, "/test2", "Op 2"),
        ];

        let results = batch.execute_batch(operations, &tools, "zeroclaw_user", true).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }
}
