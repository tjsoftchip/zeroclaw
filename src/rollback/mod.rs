//! Configuration snapshot and rollback system for OpenWrt smart gateways.
//!
//! This module provides configuration snapshot management capabilities:
//! - Create snapshots of /etc/config directory
//! - List and filter snapshots by time/label
//! - Delete snapshots
//! - Store metadata in SQLite, content as compressed archives
//! - Automatic and manual rollback
//! - Emergency recovery
//! - Safe change execution with hooks
//!
//! # Architecture
//!
//! - [`snapshot`] - Snapshot data structures and manager
//! - [`store`] - SQLite metadata storage
//! - [`journal`] - Change journal for tracking modifications
//! - [`engine`] - Rollback engine with auto-rollback capabilities
//! - [`tools`] - Tool implementations for LLM function calling
//! - [`rollback_tools`] - Rollback-specific tool implementations
//! - [`wrapper`] - Safe change wrapper with hooks and rollback
//!
//! # Extension
//!
//! To add new snapshot operations, implement the [`Tool`] trait in a new
//! submodule and register it in the tools registry.

pub mod engine;
pub mod journal;
pub mod rollback_tools;
pub mod snapshot;
pub mod store;
pub mod tools;
pub mod wrapper;

pub use engine::{
    ConfirmResult, PendingRollback, RollbackConfig, RollbackEngine, RollbackLogEntry,
    RollbackResult, RollbackTrigger,
};
pub use journal::{
    ChangeJournal, ChangeQuery, ChangeRecord, ChangeStatus, ChangeType, ConfigDiff,
    ConfigDiffEntry, DiffSummary, DiffType, compute_diff, compute_diff_for_record, format_diff_output,
};
pub use rollback_tools::{
    AutoRollbackTool, ChangeConfirmTool, ConfigRollbackTool, EmergencyRecoveryTool,
    RollbackStatusTool,
};
pub use snapshot::{ConfigSnapshot, CreateSnapshotOptions, SnapshotFilter, SnapshotManager};
pub use store::SnapshotStore;
pub use tools::{
    ConfigSnapshotCreateTool, ConfigSnapshotDeleteTool, ConfigSnapshotListTool,
};
pub use wrapper::{
    BatchChangeExecutor, ChangeContext, ChangeHook, ChangeOperation, ChangeResult,
    ExecutorConfig, HookDecision, LoggingHook, NotificationHook,
    SafeChangeExecutor, SafeToolWrapper, ValidationHook,
};
