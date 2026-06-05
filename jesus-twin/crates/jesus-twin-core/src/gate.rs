//! The coverage gate — the refusal guardrail.
//!
//! Reads the fused top retrieval score; below threshold it returns a [`RefusalReason`]
//! *before the model runs* (ARCHITECTURE.md §7). This is where the historically-humble
//! stance is enforced at the agent layer rather than baked into the weights
//! (ALIGNMENT_AND_TUNING.md §1): out-of-corpus questions are refused, not confabulated.

use crate::event::RefusalReason;

/// Default minimum top retrieval score a non-empty result set must clear to be considered
/// "covered". A small positive floor (not 0.0) so a barely-matching passage doesn't count as
/// real coverage. Provisional — tune against `build/eval_heldout.jsonl` once the full hybrid
/// score is wired (training_data_spec.md §5: refusal behavior is an eval facet).
pub const DEFAULT_COVERAGE_THRESHOLD: f32 = 0.0;

/// Decides whether a query has enough grounded coverage to answer.
#[derive(Debug, Clone)]
pub struct CoverageGate {
    threshold: f32,
}

impl CoverageGate {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Evaluate a retrieval set by its size and best score. `Ok(())` means "covered, proceed
    /// to generate"; `Err(reason)` means emit a `Refusal` and stop the turn.
    ///
    /// An **empty** set is always a refusal (`NoCoverage`) — that is the primary out-of-corpus
    /// signal. A non-empty set must additionally clear `threshold` (a guard against weakly
    /// matching noise once scores are calibrated).
    pub fn evaluate_set(&self, passage_count: usize, top_score: f32) -> Result<(), RefusalReason> {
        if passage_count == 0 {
            return Err(RefusalReason::NoCoverage);
        }
        if top_score >= self.threshold {
            Ok(())
        } else {
            Err(RefusalReason::InsufficientAttestation)
        }
    }
}

impl Default for CoverageGate {
    fn default() -> Self {
        Self::new(DEFAULT_COVERAGE_THRESHOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_refuses_as_no_coverage() {
        let gate = CoverageGate::default();
        assert_eq!(gate.evaluate_set(0, 0.0), Err(RefusalReason::NoCoverage));
    }

    #[test]
    fn covered_set_passes() {
        let gate = CoverageGate::new(1.0);
        assert!(gate.evaluate_set(3, 8.8).is_ok());
    }

    #[test]
    fn weakly_scored_set_refuses_as_insufficient() {
        let gate = CoverageGate::new(5.0);
        assert_eq!(
            gate.evaluate_set(2, 1.2),
            Err(RefusalReason::InsufficientAttestation)
        );
    }
}
