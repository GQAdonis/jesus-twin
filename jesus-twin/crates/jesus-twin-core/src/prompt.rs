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

/// The Tier-2 (low-confidence) handling addendum, appended to the grounding block when only a
/// single retrieval leg matched (`CoverageGate` → `Coverage::LowConfidence`). It instructs the
/// model to speak to what the passages genuinely cover and, in voice, decline what they do not —
/// the honest-hedge mitigation for weakly-grounded turns.
///
/// This is a **per-turn context injection, NOT a SYSTEM_PROMPT edit** — it must never be added to
/// any of the four synchronized SYSTEM_PROMPT locations (`prompt.rs`, `build_training_jsonl.py`,
/// `ollama/Modelfile.jesus-twin`, `PROMPTS.md`), so train/inference parity stays intact.
pub const LOW_CONFIDENCE_ADDENDUM: &str = "[These passages only partly touch what is asked. \
Speak plainly to what they genuinely cover, and where they do not reach the question, say so \
in your own voice rather than reaching beyond them.]";

/// Assemble the episodic-memory block (episodic-memory): what the mentor remembers about THIS
/// person from earlier conversations — facts about the user/relationship ONLY, never doctrine or a
/// belief about the world. Injected per-turn before the grounding block (a context injection, NOT
/// a SYSTEM_PROMPT edit). Empty memories → empty block.
pub fn assemble_memory_block(memories: &[String]) -> String {
    let items: Vec<String> = memories
        .iter()
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .map(|m| format!("- {m}"))
        .collect();
    if items.is_empty() {
        return String::new();
    }
    format!(
        "[What you remember about this person from earlier (facts about them, not new teaching):\n{}]",
        items.join("\n")
    )
}

/// Provenance label for the Tanakh **source-material** block (hebrew-bible). These are the Hebrew
/// scriptures the mentor DREW ON — what he read and reasoned from — **not his own words**. The
/// label is load-bearing: it keeps "his source material" distinct from "his teaching", so a quoted
/// scripture is never rendered as one of his sayings (the bright line). Placed before the grounding
/// block, which stays at the high-attention end-of-prompt position.
pub const SOURCE_INSTRUCTION: &str = "[For reference, passages from the Hebrew scriptures you drew \
on — your source material, what you read and reasoned from, not your own teaching. You may cite or \
build on them as yourself, but keep them distinct from your own words.]";

/// Assemble Tanakh source-material lines into a labeled block (hebrew-bible). Empty input → empty
/// block (no label without source to govern). The caller passes `ref: text` lines from
/// [`jesus_twin_store::SourcePassage`]s, never the red-letter corpus.
pub fn assemble_source_block(passages: &[String]) -> String {
    if passages.is_empty() {
        return String::new();
    }
    format!("{SOURCE_INSTRUCTION}\n{}", passages.join("\n"))
}

/// Provenance label for the Gospel-**narrative** block (gospel-context-kb). These are what the
/// record shows the mentor DID — his deeds and the situations around them, as others recorded them
/// — **not his words**. The label keeps "what he did" distinct from "what he said", so a narrator's
/// description is never spoken as one of his sayings.
pub const NARRATIVE_INSTRUCTION: &str = "[For context, what the record shows you did in moments like \
this — your actions and the situations around them, as others recorded them. These are deeds, not \
your words; let them inform how you respond, but never speak a narrator's description as your own \
teaching.]";

/// Assemble Gospel-narrative lines into a labeled block (gospel-context-kb). Empty input → empty
/// block. The caller passes `ref: text` lines from [`jesus_twin_store::NarrativePassage`]s.
pub fn assemble_narrative_block(passages: &[String]) -> String {
    if passages.is_empty() {
        return String::new();
    }
    format!("{NARRATIVE_INSTRUCTION}\n{}", passages.join("\n"))
}

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

/// Like [`assemble_context`], but for a Tier-2 (low-confidence) turn: the grounding block with
/// the [`LOW_CONFIDENCE_ADDENDUM`] appended at the end (the high-attention end-of-prompt
/// position). Empty passages yield an empty block — a no-coverage turn refuses and never reaches
/// generation, so there is nothing to hedge.
pub fn assemble_context_low_confidence(passages: &[String]) -> String {
    let base = assemble_context(passages);
    if base.is_empty() {
        return String::new();
    }
    format!("{base}\n\n{LOW_CONFIDENCE_ADDENDUM}")
}

/// The Tier-2 **principle-bridging** addendum head (principle-tier). Used when the retrieved
/// passages carry machine-tagged `principles` facets: instead of merely declining, the model
/// names the governing principle the passages establish and speaks to how it bears on the asked
/// (adjacent) situation — then says where the record stops. Per-turn context only; NOT a
/// SYSTEM_PROMPT edit (see the parity note on [`LOW_CONFIDENCE_ADDENDUM`]).
pub const PRINCIPLE_BRIDGING_HEAD: &str = "[These passages don't address the exact situation, but \
they establish a principle that bears on it. Speak to that principle plainly and in your own \
voice, then say where the record stops rather than reaching past it. The principle(s) at stake:";

/// Like [`assemble_context_low_confidence`], but bridges via the supplied governing `principles`
/// (the retrieved passages' facets). Empty passages → empty block; empty principles → fall back to
/// the plain low-confidence hedge (so callers can pass through unconditionally).
pub fn assemble_context_principle_tier(passages: &[String], principles: &[String]) -> String {
    let base = assemble_context(passages);
    if base.is_empty() {
        return String::new();
    }
    if principles.is_empty() {
        return format!("{base}\n\n{LOW_CONFIDENCE_ADDENDUM}");
    }
    let list = principles
        .iter()
        .map(|p| format!("- {}", p.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{base}\n\n{PRINCIPLE_BRIDGING_HEAD}\n{list}]")
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
        assert_eq!(assemble_context_low_confidence(&[]), "");
    }

    #[test]
    fn principle_tier_bridges_via_principles() {
        let p = ["Mark 12:29-31: Hear, Israel...".to_string()];
        let block = assemble_context_principle_tier(
            &p,
            &["Love of God and neighbor is the whole of the law.".to_string()],
        );
        assert!(block.contains("Mark 12:29-31"), "keeps the grounding block");
        assert!(
            block.contains(PRINCIPLE_BRIDGING_HEAD),
            "uses the bridging head"
        );
        assert!(
            block.contains("Love of God and neighbor is the whole of the law."),
            "names the principle"
        );
    }

    #[test]
    fn principle_tier_empty_principles_falls_back_to_hedge() {
        let p = ["Mark 12:29-31: Hear, Israel...".to_string()];
        let block = assemble_context_principle_tier(&p, &[]);
        assert!(block.trim_end().ends_with(LOW_CONFIDENCE_ADDENDUM));
        assert_eq!(block, assemble_context_low_confidence(&p));
    }

    #[test]
    fn low_confidence_appends_addendum_after_passages() {
        let p = ["Mark 12:29-31: Hear, Israel...".to_string()];
        let block = assemble_context_low_confidence(&p);
        assert!(
            block.starts_with(CONTEXT_INSTRUCTION),
            "keeps the grounding block"
        );
        assert!(block.contains("Mark 12:29-31"), "keeps the passages");
        assert!(
            block.trim_end().ends_with(LOW_CONFIDENCE_ADDENDUM),
            "addendum is appended at the end-of-prompt position"
        );
    }

    #[test]
    fn system_prompt_declares_context_is_not_user_supplied() {
        assert!(
            SYSTEM_PROMPT.contains("the person asking has not presented them"),
            "system contract must declare grounding passages are not a user submission"
        );
    }
}
