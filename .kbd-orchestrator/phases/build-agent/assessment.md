# Build Agent Assessment

**Phase:** build-agent  
**Date:** 2026-06-05  
**Status:** assessment_complete

---

## Sycophancy Correction Result

**Score:** 0.0 (clean)  
**Patterns detected:** None  
**Assessment:** The build goal is stated plainly without overclaiming speed or underestimating effort. No correction needed.

---

## Current State Inventory

### Infrastructure (READY)

| Component | Status | Evidence |
|-----------|--------|----------|
| Rust workspace | ✅ Scaffolded | 7 crates in `jesus-twin/crates/` |
| MistralEngine | ✅ Done | Compile-verified, engine-pluggable serve |
| MockEngine | ✅ Done | RAG-first testing works |
| Hybrid retrieval | ✅ Done | BM25 + HNSW + RRF in Rust |
| Mindmap skill | ✅ Done | Graph projection implemented |
| MCP skill registry | ✅ Done | list_skills / invoke_skill exposed |
| Data pipeline | ✅ Done | `build_training_jsonl.py` produces all 3 outputs |
| RAG corpus | ✅ Done | 927 rows in `build/rag_corpus.jsonl` |

### Data (BLOCKED)

| Dataset | Status | Rows | Blocker |
|---------|--------|------|---------|
| `build/rag_corpus.jsonl` | ✅ Ready | 927 | — |
| `build/sft_style.jsonl` | ❌ Empty | 0 | Annotation required |
| `build/eval_heldout.jsonl` | ❌ Empty | 0 | Annotation required |

### Annotation (THE BOTTLENECK)

The `jesus_full_red_letter.xlsx` has 927 sayings extracted but **zero rows annotated** with:
- `Modern Rendering` (present-day English, preserving force)
- `Reasoning Move` (M01–M18 tag)

**This is the only blocker to a working mentor agent.**

---

## Gap Analysis Against VISION.md

### What VISION.md Requires

1. **Conversational mentor persona** — warm, direct, personally engaged
2. **Nine rhetorical methods** — parable, *kal v'homer*, counter-question, *remez*, contrast, inversion, personal address, rule of three, incremental extension
3. **Hebrew Bible integration** — as his source material, not foreign content
4. **Graceful refusal** — in-character, not system messages
5. **Attestation-aware responses** — confidence tracks evidence quality

### What's Missing

| Requirement | Current State | Gap |
|-------------|---------------|-----|
| Mentor warmth/directness | System prompt only | Needs LoRA training on conversational examples |
| Rhetorical methods | M01–M18 rubric exists | Needs method-labeled annotation + SFT |
| Hebrew Bible retrieval | Not implemented | Needs Tanakh corpus + RAG integration |
| In-voice refusal | Designed in ARCHITECTURE.md | Needs implementation + testing |
| Attestation metadata | Designed in ALIGNMENT_AND_TUNING.md | Needs annotation schema extension |

---

## Build Plan (Sycophancy-Corrected)

### Honest Timeline

**Days, not weeks:**
- Test RAG-first system with MockEngine
- Validate retrieval quality and refusal behavior
- Ship first working prototype (no voice, just grounded answers)

**Weeks, not days:**
- Annotate 50–100 sayings with method labels
- Build first SFT dataset (conversational mentor examples)
- Train first LoRA, evaluate, iterate

**Months, not weeks:**
- Scale annotation to 300+ rows
- Train production LoRA with full method repertoire
- Deploy real mentor agent

### Why This Order

1. **RAG-first validates the truth layer** before adding voice. If retrieval is broken, no amount of fine-tuning fixes it.

2. **Annotation is human work, not code.** The Rust infrastructure is ready. The blocker is careful, methodical annotation that captures his methods, not just surface rendering.

3. **Small LoRA first, then scale.** A 50-row LoRA will show whether the method labels are working. If the model can't learn parable vs. counter-question from 50 examples, 500 won't help — the annotation schema needs revision.

4. **Hebrew Bible is a separate workstream.** It requires its own corpus preparation and RAG integration. Don't block the mentor on it.

---

## Minimum Viable Milestone

**Goal:** A working conversational agent that retrieves cited sayings and refuses out-of-corpus questions in-character.

**Deliverables:**
1. ✅ Rust service running with MockEngine
2. ✅ Hybrid retrieval returning top-k sayings with citations
3. ✅ Coverage gate refusing low-confidence queries
4. ✅ In-voice refusal messages (not system errors)
5. ✅ Basic conversational warmth from system prompt

**What this is NOT:**
- Not a convincing mentor (no voice fine-tune yet)
- Not method-aware (no rhetorical pattern training)
- Not Hebrew-Bible-integrated (Tanakh not loaded)

**Effort estimate:** 2–3 days of integration work + testing

---

## Concrete Next Actions

### Immediate (This Week)

1. **Test RAG-first prototype**
   - Run `cargo run --bin jesus-twin -- serve --db ./twin.db`
   - Test retrieval with 10 sample queries
   - Test refusal with 5 out-of-corpus queries
   - Validate citation format and in-voice refusal
   - **Effort:** 4 hours

2. **Write annotation guide** (`docs/annotation-guide.md`)
   - Define method labels (parable, *kal v'homer*, etc.)
   - Provide 5 positive examples, 5 negative examples
   - Specify Modern Rendering style (warm, direct, not sentimental)
   - **Effort:** 6 hours

### Short-Term (Next 2 Weeks)

3. **Annotate 50 representative sayings**
   - Include all 9 method types
   - Include synoptic controversy, parables, aphorisms, prayer, Johannine material
   - Run `build_training_jsonl.py` to produce first SFT dataset
   - **Effort:** 20–30 hours (human work)

4. **Build 25 conversational mentor examples**
   - Personal questions (anxiety, money, relationships, purpose)
   - Answers using his documented methods
   - Blend into SFT dataset
   - **Effort:** 10 hours

5. **Train first LoRA (50–75 rows)**
   - Use Unsloth on Colab/Kaggle (free GPU)
   - Evaluate on held-out set
   - Check method-application fidelity (does it use parable when appropriate?)
   - **Effort:** 8 hours (mostly waiting for training)

### Medium-Term (Next Month)

6. **Iterate on annotation based on LoRA results**
   - If method labels aren't working, revise schema
   - If warmth is missing, add more conversational examples
   - Scale to 150–200 rows

7. **Add Hebrew Bible source tool**
   - Prepare Tanakh corpus (public domain JPS 1917)
   - Integrate into RAG as separate retrieval path
   - Test *remez* allusions in responses

8. **Train production LoRA (300+ rows)**
   - Full method repertoire
   - Conversational warmth
   - Deploy to real MistralEngine

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Annotation takes longer than expected | High | Medium | Start with 50 rows, validate before scaling |
| Method labels don't transfer to LoRA | Medium | High | Test with small LoRA first, revise schema if needed |
| Warmth target slides into sycophancy | Medium | Medium | DPO on "direct but uncomfortable truth" ≻ "agreeable platitude" |
| Hebrew Bible integration is complex | Medium | Low | Defer until mentor core is working |
| GPU access for training | Low | High | Use free Colab/Kaggle; document local GPU path |

---

## Progress Update

```json
{
  "phase": "build-agent",
  "assessment_complete": true,
  "infrastructure_ready": true,
  "data_pipeline_ready": true,
  "annotation_ready": false,
  "rag_prototype_ready": false,
  "lora_training_ready": false,
  "next_milestone": "rag-first-prototype",
  "blocker": "annotation",
  "estimated_effort_to_mvp": "2-3 days integration + 2-3 weeks annotation"
}
```

---

## Bottom Line

The Rust infrastructure is **ready**. The data pipeline is **ready**. The only blocker is **annotation** — careful, methodical human work that captures his rhetorical methods, not just surface rendering.

**Do not skip the RAG-first prototype.** It validates the truth layer before adding voice. If retrieval is broken, no fine-tune fixes it.

**Do not skip the small LoRA test.** A 50-row LoRA will show whether the method labels work. If they don't, revise the schema before scaling to 500 rows.

**Do not overpromise speed.** A convincing mentor requires hundreds of annotated examples and will take weeks of careful work. A grounded answer engine can be built in days.

The path is clear: RAG-first → annotate 50 → small LoRA → iterate → scale.

---

# Assessment addendum — Optimal-method validation + base-model choice (2026-06-09)

> Scope: is the hybrid approach actually the best way to build the Jesus-mind mentor agent,
> compared against the method families in the role-play/persona literature? Plus: would
> Qwen 3.x have been a better fine-tune base than Gemma 4? Web-researched (Firecrawl);
> verdict passed the sycophancy detector at **adversarial** strictness (score 0.0).

## The four method families (from the persona/role-play literature)

The surveys (R-CHAR EMNLP 2025; "Two Tales of Persona" EMNLP 2024; "Oscars of AI Theater"
arXiv:2407.11484) split character simulation into: in-context learning, character-specific
fine-tuning, and training-free agentic/memory architectures — plus this project's hybrid.

| Method | What it is | Strengths | Fatal weakness for THIS goal |
|---|---|---|---|
| **A. This project: hybrid** | RAG owns truth (927 cited passages), light style-LoRA owns voice, agent layer owns stance/refusal; small local model | Auditability; citations; refusal-on-no-coverage; local sovereignty; stance changeable without retrain | Voice quality capped until the LoRA lands (annotation-gated); engineering cost |
| **B. Frontier prompt-persona (+RAG)** — the "Text With Jesus" approach | System prompt + retrieval on a GPT/Claude-class API | Best raw fluency/warmth today; days to ship; no training | Truth/stance live in a provider's opaque weights + a prompt; persona drifts with provider updates; per-token cost; weak refusal auditability. Public criticism of these apps centers on exactly this (NYT 2025-11; blasphemy/accuracy complaints) |
| **C. Deep persona fine-tune** (CharacterLLM / CharacterBot, arXiv:2502.12988) | Bake linguistic patterns AND thought processes into weights, multi-task | Strongest stylistic immersion in benchmarks | Moves *truth into weights* — unauditable; fabrication risk is the cardinal sin here. CharacterBot's case study (Lu Xun) had 17 essay collections; this corpus is 927 verse-rows — an order of magnitude less signal |
| **D. Agentic memory/metacognition** (R-CHAR, Human Simulacra, generative agents) | No weight changes; persona via structured memory + episodic retrieval + metacognitive scenario reasoning | Psychological depth; relationship memory | No public case on an ancient, contested, citation-required corpus; orchestration complexity; faithfulness benchmarks still favor explicit grounding |

## Verdict (sycophancy-checked)

**The hybrid (A) is the best fit for THIS project's stated constraints** — never fabricate,
cite everything, auditability, local-first, stance enforced outside the weights (VISION.md §1–2).
**It is NOT universally optimal:** under a different constraint set ("maximum perceived
warmth/fluency at minimum effort"), **B wins today**, and the honest position is that this
project's differentiator is the *honesty architecture* (coverage gate, citations, attestation
tiers) — not model quality. C is rejected on evidence-integrity grounds, not fashion. D is
worth *borrowing from* — an episodic memory of the user relationship would strengthen the
mentor experience — without adopting wholesale.

Supporting evidence already in repo memory: RAG beats FT for facts by ~37pp and FT+RAG can
underperform RAG alone (arXiv:2312.05934); style LoRA reaches >90% on-target tone at ~100
samples vs 23–46% for prompting (arXiv:2507.04889). The split "retrieval=truth /
fine-tune=voice / agent=stance" is the configuration the evidence supports.

## Qwen 3.x vs Gemma 4 as the fine-tune base

**What Gemma 4 actually cost us (all documented in-repo):**
- `Gemma4ForConditionalGeneration` is a VLM class → `TextModelBuilder` rejected it; required
  the `MultimodalModelBuilder` switch (feat/cuda-rag-release).
- Broken mmproj GGUF conversion → bypass via direct `llama-quantize` (commit cee0f92).
- Merge-only LoRA (no runtime adapter in vLLM/SGLang/mistral.rs) — CLAUDE.md gotcha.
- Chat-template fragility (`gemma-4` vs `gemma-4-thinking`; turn-marker mismatch = gibberish).
- Community: "Gemma 4 is seriously broken when using Unsloth and llama.cpp" (r/LocalLLaMA,
  cited in docs/training-and-deployment-guide.md §10).

**What Qwen3-4B offers:** first-class Unsloth support (official docs + notebooks; Unsloth
works directly with the Qwen3 team), official `unsloth/Qwen3-4B-GGUF` exports, text-only
causal-LM architecture (no VLM builder complications), runtime LoRA support in major serving
stacks, stable ChatML template. Same thinking-mode-off discipline applies (Qwen3 hybrid
thinking → `enable_thinking=False`).

**The honest limit of the comparison:** the fine-tune collapse was caused by **75 rows at
lr 2e-4 × 3 epochs** — a data/recipe failure that would have collapsed Qwen3-4B identically.
Switching bases would have saved *toolchain* pain, not the fine-tune. Base-model choice and
collapse cause are independent variables; conflating them would be the wrong lesson.

**Recommendation:**
1. **Keep Gemma 4 E4B for the operational serving build** — it is shipped, working, and the
   VLM/GGUF workarounds are paid costs.
2. **At retrain time (≥300 annotated rows per the 2026-06-09 assessment), train BOTH
   Qwen3-4B and Gemma 4 E4B** with the gentle recipe (lr ~2e-5, 1 epoch) and let the
   existing 145-test eval suite decide on style-by-move-without-grounding-loss.
3. **Default the first retrain attempt to Qwen3-4B** on tooling-maturity grounds; its
   runtime-LoRA support also removes the merge-only constraint, shortening iteration loops.

## Gaps surfaced by this assessment

- **No episodic user-relationship memory** (Method D's genuine contribution) — candidate
  future change; fits the existing SurrealDB store.
- **The differentiator is under-communicated**: citations/attestation/refusal honesty is the
  product vs the prompt-persona apps; surface it in UI (the AG-UI custom chunks) and docs.
- **Eval suite is the deciding instrument** for every contested choice above — keep it the
  gate for base-model selection and LoRA acceptance.
