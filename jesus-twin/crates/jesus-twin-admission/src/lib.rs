//! Admission control — the scheduler boundary (ARCHITECTURE.md §6).
//!
//! Sits **in front of** the inference engine and decides *whether and when* a request enters
//! it (admit / queue / backpressure / 503). It does NOT do per-token GPU scheduling or task
//! execution — that's mistral.rs's internal engine. Keep these layers strictly separate
//! (CLAUDE.md gotcha).
//!
//! Contract: `gatekeeper.admit(cost) -> Permit`. The orchestrator holds the `Permit` (RAII)
//! for the whole generation and drops it on finish, releasing the reserved units.
//!
//! Two implementations: [`OpenGatekeeper`] (always admits, for the single-user edge build)
//! and [`SemaphoreGatekeeper`] (bounded concurrency + queue-depth backpressure).

pub mod gatekeeper;

pub use gatekeeper::{
    AdmissionError, Cost, Gatekeeper, OpenGatekeeper, Permit, SemaphoreGatekeeper,
};
