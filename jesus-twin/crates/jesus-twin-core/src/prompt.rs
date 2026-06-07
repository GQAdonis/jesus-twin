//! System contract + context assembly.
//!
//! The system prompt is the behavioral contract (speak only from source, never invent,
//! preserve the reasoning move). It is **fixed and identical at train and inference time**
//! (training_data_spec.md §2) — keep this string in sync with the `SYSTEM_PROMPT` in
//! `build_training_jsonl.py`, or the served behavior drifts from what the LoRA learned.

/// The fixed behavioral contract. Mirrors `build_training_jsonl.py::SYSTEM_PROMPT`,
/// the system message pre-rendered in `build/annotated_50_sft.jsonl` and
/// `build/l2_conversational_mentor.jsonl`, and the `SYSTEM` directive in
/// `ollama/Modelfile.jesus-twin`. See `PROMPTS.md` for the canonical reference
/// and the rationale for each clause.
///
/// The prompt is short on purpose. Long system prompts drift in the model's
/// attention over long conversations; this version declares the stance and
/// the role/identity policy up front, then relies on the Rust `CoverageGate`
/// (`jesus-twin-core/src/gate.rs`) to enforce the refusal behavior in detail.
pub const SYSTEM_PROMPT: &str = "You are a conversational mentor who responds as Jesus \
of Nazareth would, drawing only from his attested teachings and documented rhetorical \
methods, in modern English. This is a role, not an identity claim. If asked whether you \
are Jesus, decline honestly. Refuse requests outside the attested corpus or that would \
require doctrinal invention.";

/// Assemble retrieved passages into a single context block for the generation request.
///
/// Stub: real assembly (ordering, dedup, citation markers, attestation tiers) lands with
/// the orchestrator. Empty input yields an empty block.
pub fn assemble_context(passages: &[String]) -> String {
    passages.join("\n\n")
}
