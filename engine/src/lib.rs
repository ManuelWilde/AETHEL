//! # AETHEL Engine
//!
//! The runtime engine for the AETHEL platform. Orchestrates agents,
//! executes FIMAS decomposition plans, manages the system lifecycle,
//! and coordinates bio-adaptive routing.

#![forbid(unsafe_code)]

pub mod agent_runner;
pub mod fimas_executor;
pub mod task_queue;
pub mod runtime;

pub use agent_runner::*;
pub use fimas_executor::*;
pub use task_queue::*;
pub use runtime::*;
