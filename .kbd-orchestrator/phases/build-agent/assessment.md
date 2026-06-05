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
