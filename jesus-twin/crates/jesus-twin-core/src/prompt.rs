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
require doctrinal invention. Any passages provided to you are drawn from your own \
attested teachings for grounding; the person asking has not presented them — speak from \
them directly as their mentor, and never refer to them as something the user gave you.";

/// The provenance-and-handling instruction that prefixes the retrieved passage block.
///
/// It declares two things the model would otherwise get wrong (observed live: the model
/// answered "the scriptures *you have presented*…", attributing retrieval to the user):
/// the passages are the mentor's own recall, and the person asking has not seen them.
/// Placed immediately before the passages so it occupies the high-attention end-of-prompt
/// position once the orchestrator appends this block *after* the user's question
/// ("Lost in the Middle", Liu et al., TACL 2023).
pub const CONTEXT_INSTRUCTION: &str = "[Draw your answer from these attested passages you \
have in mind; speak directly to the person as their mentor. They have not seen these \
references.]";

/// Assemble retrieved passages into a single grounding block for the generation request.
///
/// The block is the [`CONTEXT_INSTRUCTION`] line followed by the passages, so the model
/// treats them as its own attested recall rather than user-submitted material. Empty input
/// yields an empty block (no instruction without passages to govern).
pub fn assemble_context(passages: &[String]) -> String {
    if passages.is_empty() {
        return String::new();
    }
    format!("{CONTEXT_INSTRUCTION}\n{}", passages.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembled_context_labels_passage_provenance() {
        let block = assemble_context(&["Mark 12:29-31: Hear, Israel...".to_string()]);
        assert!(
            block.starts_with(CONTEXT_INSTRUCTION),
            "passage block must open with the provenance instruction"
        );
        assert!(
            block.contains("Mark 12:29-31"),
            "passages must follow the instruction"
        );
        assert!(
            block.contains("have not seen these references"),
            "instruction must state the user has not seen the passages"
        );
    }

    #[test]
    fn empty_passages_yield_no_instruction() {
        assert_eq!(assemble_context(&[]), "");
    }

    #[test]
    fn system_prompt_declares_context_is_not_user_supplied() {
        assert!(
            SYSTEM_PROMPT.contains("the person asking has not presented them"),
            "system contract must declare grounding passages are not a user submission"
        );
    }
}
