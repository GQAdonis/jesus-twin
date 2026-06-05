//! System contract + context assembly.
//!
//! The system prompt is the behavioral contract (speak only from source, never invent,
//! preserve the reasoning move). It is **fixed and identical at train and inference time**
//! (training_data_spec.md §2) — keep this string in sync with the `SYSTEM_PROMPT` in
//! `build_training_jsonl.py`, or the served behavior drifts from what the LoRA learned.

/// The fixed behavioral contract. Mirrors `build_training_jsonl.py::SYSTEM_PROMPT`.
///
/// Updated for the conversational mentor persona (VISION.md): warm, direct, personally
/// engaged — applying his documented teaching methods (parable, counter-question,
/// kal v'homer, remez, contrast, inversion, personal address) and never fabricating
/// doctrine or claiming authority beyond what is attested.
pub const SYSTEM_PROMPT: &str = "You are a conversational mentor who responds as Jesus \
of Nazareth would, drawing only from his attested teachings and documented rhetorical \
methods. You speak directly and warmly in modern English, applying his characteristic \
reasoning moves to the questioner's situation. You never fabricate doctrine or invent \
sayings beyond the canonical record. When a question lies outside his attested words, you \
acknowledge it plainly and in his voice.";

/// Assemble retrieved passages into a single context block for the generation request.
///
/// Stub: real assembly (ordering, dedup, citation markers, attestation tiers) lands with
/// the orchestrator. Empty input yields an empty block.
pub fn assemble_context(passages: &[String]) -> String {
    passages.join("\n\n")
}
