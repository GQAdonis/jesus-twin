//! The coverage gate — the refusal / low-confidence guardrail.
//!
//! Classifies a retrieval set by **leg agreement** (how many independent retrieval legs ranked
//! the top passage), not raw RRF score, into three tiers (ARCHITECTURE.md §7; the
//! historically-humble stance is enforced here at the agent layer, not baked into the weights —
//! ALIGNMENT_AND_TUNING.md §1). The tiers and the empirical calibration behind them are recorded
//! in `docs/FINDINGS.md` (gate-calibration change):
//!
//! - **Tier 1 / `Grounded`** — ≥2 legs agreed: lexical and semantic retrieval independently
//!   surfaced the same passage. Answer normally.
//! - **Tier 2 / `LowConfidence`** — exactly 1 leg: a single-modality match (semantic-only, or the
//!   BM25-only fallback). Answer, but with the low-confidence honesty signal + an in-voice
//!   instruction to decline what the passages don't cover.
//! - **Tier 3 / `NoCoverage`** — empty / stopword-only: out-of-corpus. Refuse before the model runs.
//!
//! Why leg agreement and not a score floor: the calibration showed grounding and out-of-corpus
//! RRF scores overlap (non-separable), while leg agreement is deterministic, human-explainable,
//! and robust to the annotation program reviving the currently-dead modern legs — no recalibration
//! at the ~300-row milestone.

/// The coverage tier for a retrieval set — the gate's three-outcome verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// ≥2 retrieval legs agreed on the top passage. Answer normally.
    Grounded,
    /// Exactly 1 leg matched (single-modality). Answer with the low-confidence signal + hedge.
    LowConfidence,
    /// No passage retrieved (empty / stopword-only). Refuse — out-of-corpus.
    NoCoverage,
}

/// Classifies a retrieval set into a [`Coverage`] tier by leg agreement. Stateless: the rule is
/// fixed by `docs/FINDINGS.md` and carries no tunable threshold (the old `DEFAULT_COVERAGE_THRESHOLD`
/// was structurally disabled — RRF scores are strictly positive, so it passed everything).
#[derive(Debug, Clone, Default)]
pub struct CoverageGate;

impl CoverageGate {
    pub fn new() -> Self {
        Self
    }

    /// Classify by passage count and the leg agreement of the top passage. An empty set is always
    /// [`Coverage::NoCoverage`]; otherwise ≥2 legs → [`Coverage::Grounded`], else
    /// [`Coverage::LowConfidence`].
    pub fn classify(&self, passage_count: usize, top_legs_matched: u8) -> Coverage {
        if passage_count == 0 {
            Coverage::NoCoverage
        } else if top_legs_matched >= 2 {
            Coverage::Grounded
        } else {
            Coverage::LowConfidence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_is_no_coverage() {
        // Tier 3: out-of-corpus / stopword-only — leg count is irrelevant when nothing matched.
        assert_eq!(CoverageGate.classify(0, 0), Coverage::NoCoverage);
        assert_eq!(CoverageGate.classify(0, 2), Coverage::NoCoverage);
    }

    #[test]
    fn two_or_more_legs_is_grounded() {
        // Tier 1 boundary: exactly 2 agreeing legs (fused ≥ 2/61) → answer normally.
        assert_eq!(CoverageGate.classify(5, 2), Coverage::Grounded);
        assert_eq!(CoverageGate.classify(1, 4), Coverage::Grounded);
    }

    #[test]
    fn single_leg_is_low_confidence() {
        // Tier 2 boundary: one leg (fused 1/61) — semantic-only or BM25-only fallback.
        assert_eq!(CoverageGate.classify(5, 1), Coverage::LowConfidence);
    }
}
