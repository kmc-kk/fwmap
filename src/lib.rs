pub mod cli;
pub mod core;
pub mod debug;
pub mod ingest;
pub mod report;
pub mod sarif;
pub mod validation;

pub use core::analyze;
pub use core::demangle;
pub use core::diff;
pub use core::history;
pub use core::linkage;
pub use core::model;
pub use core::rule_config;
pub use core::rules;
pub use report::render;
pub use sarif as sarif_report;
