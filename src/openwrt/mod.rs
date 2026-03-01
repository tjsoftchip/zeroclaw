//! OpenWrt UCI configuration system integration.
//!
//! This module provides Rust interfaces for OpenWrt's Unified Configuration
//! Interface (UCI), enabling configuration management through the Tool trait.
//!
//! # Architecture
//!
//! - [`types`] - Core UCI data structures (UciConfig, UciValue, UciTransaction)
//! - [`executor`] - UCI command execution wrapper
//! - [`transaction`] - Transaction management with rollback support
//! - [`uci_get`] - Tool for reading UCI configuration
//! - [`uci_set`] - Tool for writing UCI configuration
//! - [`uci_list`] - Tool for listing UCI configuration
//! - [`uci_validate`] - Tool for validating UCI configuration
//!
//! # Extension
//!
//! To add new UCI operations, implement the [`Tool`] trait in a new submodule
//! and register it in the tools registry.

pub mod executor;
pub mod transaction;
pub mod types;
pub mod uci_get;
pub mod uci_list;
pub mod uci_set;
pub mod uci_validate;

pub use executor::UciExecutor;
pub use transaction::{TransactionManager, TransactionState};
pub use types::{
    UciAction, UciChange, UciConfig, UciListEntry, UciTransaction, UciValue, UciValidationResult,
};
pub use uci_get::{UciGetBulkTool, UciGetTool};
pub use uci_list::{UciChangesTool, UciListTool};
pub use uci_set::{UciBatchTool, UciSetTool};
pub use uci_validate::{UciAddTool, UciCommitTool, UciRevertTool, UciValidateTool};

use std::sync::Arc;

use crate::rollback::RollbackEngine;
use crate::security::SecurityPolicy;

pub const UCI_CONFIG_DIR: &str = "/etc/config";
pub const UCI_COMMAND: &str = "uci";

pub struct UciContext {
    pub security: Arc<SecurityPolicy>,
    pub rollback_engine: Option<Arc<RollbackEngine>>,
    pub config_dir: std::path::PathBuf,
    pub dry_run: bool,
}

impl UciContext {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self {
            security,
            rollback_engine: None,
            config_dir: std::path::PathBuf::from(UCI_CONFIG_DIR),
            dry_run: false,
        }
    }

    pub fn with_rollback(mut self, engine: Arc<RollbackEngine>) -> Self {
        self.rollback_engine = Some(engine);
        self
    }

    pub fn with_config_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.config_dir = dir;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

pub fn create_uci_tools(
    security: Arc<SecurityPolicy>,
) -> Vec<Box<dyn crate::tools::traits::Tool>> {
    let executor = Arc::new(UciExecutor::new(security.clone()));
    let transaction_manager = Arc::new(TransactionManager::new(executor.clone()));

    vec![
        Box::new(UciGetTool::new(security.clone(), executor.clone())),
        Box::new(UciGetBulkTool::new(security.clone(), executor.clone())),
        Box::new(UciSetTool::new(
            security.clone(),
            executor.clone(),
            transaction_manager.clone(),
        )),
        Box::new(UciBatchTool::new(security.clone(), transaction_manager.clone())),
        Box::new(UciListTool::new(security.clone(), executor.clone())),
        Box::new(UciChangesTool::new(security.clone(), executor.clone())),
        Box::new(UciValidateTool::new(security.clone(), executor.clone())),
        Box::new(UciCommitTool::new(security.clone(), executor.clone())),
        Box::new(UciRevertTool::new(security.clone(), executor.clone())),
        Box::new(UciAddTool::new(security, executor)),
    ]
}
