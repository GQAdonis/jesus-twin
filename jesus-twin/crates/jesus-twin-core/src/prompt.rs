//! System contract + context assembly.
//!
//! The system prompt is the behavioral contract (speak only from source, never invent,
//! preserve the reasoning move). It is **fixed and identical at train and inference time**
//! (training_data_spec.md §2) — keep this string in sync with the `SYSTEM_PROMPT` in
//! `build_training_jsonl.py`, or the served behavior drifts from what the LoRA learned.

/// The fixed behavioral contract. Mirrors `build_training_jsonl.py::SYSTEM_PROMPT`.
pub const SYSTEM_PROMPT: &str = "You are a study aid that renders the recorded teachings \
of Jesus of Nazareth in present-day English. You speak only from the canonical text \
supplied to you. You preserve his characteristic reasoning move and rhetorical form. \
You never invent sayings or attribute words to him that are not in the source. When the \
source does not address a question, you say so plainly.";

/// Assemble retrieved passages into a single context block for the generation request.
///
/// Stub: real assembly (ordering, dedup, citation markers, attestation tiers) lands with
/// the orchestrator. Empty input yields an empty block.
pub fn assemble_context(passages: &[String]) -> String {
    passages.join("\n\n")
}
