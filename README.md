# Jesus Digital Twin — Rust Edition

A Rust rebuild of the digital-twin study aid: it renders the recorded teachings of
Jesus (red-letter corpus, World English Bible) in present-day English, preserves his
reasoning moves (the `M01–M18` rubric), and **never fabricates sayings** — retrieval
grounds every answer in a cited verse.

This README captures the architecture decisions and the research behind them. The
governing principle is unchanged from the Python design: **fine-tuning teaches *form*
(voice, cadence, reasoning move); retrieval supplies *content* (what he actually said,
with a citation).** Rust changes the *implementation*, not that split.

> The concrete agent-service build spec (Axum crate layout, mistral.rs-as-library,
> the four protocol adapters, parking-lot/engine boundary, SurrealDB graphrag schema)
> lives in [`ARCHITECTURE.md`](./ARCHITECTURE.md). Tuning-layer roles (instruction tuning
> vs. fine-tuning vs. preference alignment), the non-religious historical alignment
> stance, skills/MCP tool safety, AG-UI chunks, and how much of the Bible to ingest are
> in [`ALIGNMENT_AND_TUNING.md`](./ALIGNMENT_AND_TUNING.md).
>
> Data-extraction tooling (the WEB red-letter extractor that produces the corpus) is
> documented in [`DATA_EXTRACTION.md`](./DATA_EXTRACTION.md).
>
> Research note: Firecrawl was not connected as a tool in this environment, so the web
> research below was done with the available web-search/research tooling. Treat model
> names, sizes, and VRAM figures as point-in-time (mid-2026) and re-verify before
> committing hardware spend.

---

## 1. Model choice — small models for fine-tuning

### The honest recommendation

Start with **Gemma 4 E4B** or **Qwen3-4B** (Instruct) as the primary base, and ship
**Gemma 4 E2B** (or **Llama 3.2 3B**) for the phone tier. **Gemma 4** (released
2026-04-02, Apache 2.0) is now the leading pick for this project: it adds **native
function calling / agentic tool use** and a clean Apache-2.0 license — which removes
the licensing watch-out that applied to Gemma 3. None of these is a "can't-go-wrong"
pick, but Gemma 4's tool calling specifically enables *agentic RAG* (the model drives
the SurrealDB retriever/graph itself), which fits the graphrag/mind-map direction.

| Model | Size | Why consider it | Watch-outs |
|---|---|---|---|
| **Gemma 4 E4B** | eff. 4B | **Apache 2.0**; native function calling; configurable thinking; multimodal; day-one Unsloth/MLX/llama.cpp/mistral.rs support | Very new — quantized fine-tune recipes still settling; **disable thinking mode** for diction fidelity |
| **Gemma 4 E2B** | eff. 2B | The phone tier — efficient, native audio+vision, same tool-calling/license wins | Weaker reasoning than E4B; verify on-device tooling |
| **Gemma 4 26B-A4B** | MoE, 3.8B active | Near-flagship quality, only 3.8B active params → fits a 24 GB GPU for fine-tune/inference | MoE fine-tuning has its own quirks; overkill for a tiny corpus |
| **Qwen3-4B** | 4B | Rivals much larger models on reasoning at its size; OpenAI chat-template native; the more *proven* fine-tuning path | Apache-2.0 but verify; Alibaba lineage if that matters to you |
| **Llama 3.2 3B** | 3B | Well-worn on-device / iPhone workhorse; mature MLX & llama.cpp support | Llama community license; weaker than 4B+ on hard reasoning |
| **Phi-4-mini** | 3.8B | Very strong reasoning/MMLU for size, long context | Verify license; tuned toward "helpful assistant" voice |

For "impact + fun": run **Gemma 4 E4B** (or the **26B-A4B MoE** if you have a 24 GB
GPU) for best quality with agentic tool calling, and quantize **Gemma 4 E2B** to
`Q4_K_M` for an actual on-iPhone twin. Caveat worth keeping honest: for your *tiny*
style corpus most of Gemma 4's extra capability is wasted — the license fix and tool
calling are the real reasons to prefer it, not the benchmark scores.

### Hardware reality (mid-2026 figures, QLoRA unless noted)

| Where you train | Practical ceiling | Notes |
|---|---|---|
| **NVIDIA 12 GB** (RTX 3060/4070) | QLoRA a **7–8B** model | 4-bit base + LoRA adapters ≈ 8–12 GB |
| **NVIDIA 24 GB** (RTX 4090) | **LoRA 7–8B in 16-bit** (higher quality) *or* **QLoRA 13–14B** | LoRA ≈ 90–95% of full-FT quality vs QLoRA ≈ 80–90% |
| **Mac M-series** (unified mem) | 7B QLoRA via **MLX** | Trains **3–5× slower** than a comparable NVIDIA GPU; excellent for inference |
| **iPhone / edge** | **inference of ≤3B at Q4**, not training | llama.cpp / MLX; `Gemma 3n`, `Llama 3.2 3B`, MobileLLM-class |

For this project the dataset is tiny (≈489 sayings → a few hundred training-ready
rows), so a **4B model + LoRA** is the sweet spot. A larger model would overfit this
corpus rather than help.

---

## 2. Storage / RAG — SurrealDB vs pgvector vs pglite

### The honest recommendation

Use **SurrealDB 3.1 (embedded)** as the primary store, *and* keep the vector schema
portable so you can fall back to pgvector if you ever outgrow Surreal's ANN. Reasoning:

- **SurrealDB 3.1 embedded** is the strongest fit *for this project specifically*,
  because the twin wants more than vector search — it wants **graphrag and
  mind-mapping**. SurrealQL composes **vector similarity, graph traversal, full-text /
  BM25, and RRF fusion as co-equal predicates in a single query**, and it embeds
  directly in a Rust binary (no separate server). The reasoning-move graph, the
  saying↔concept↔audience edges, and the vector index all live in one store and one
  query — exactly what graphrag and mind-mapping need.
  - *Trade-off:* SurrealDB's vector/ANN engine is younger and less battle-tested at
    scale than pgvector. At your corpus size (hundreds–thousands of vectors) this is
    irrelevant; at tens of millions it would matter.

- **pgvector on Postgres** is the more *proven* pure-vector option (comfortable to
  ~50M vectors, ACID, mature ANN). But it is **not embedded** — it needs a running
  Postgres — and has no native graph layer, so graphrag means a second system or
  recursive CTEs. Best kept as a *future* migration target if scale or recall demand it.

- **pglite** is embedded *Postgres compiled to WASM*. Interesting for local/edge, but
  it's primarily a JS/TS target, calling it cleanly from Rust is awkward, and
  pgvector-in-pglite support is limited/young. **Don't make it the primary store for a
  Rust app** — if you want embedded, SurrealDB is the more natural Rust citizen.

**Decision:** SurrealDB embedded now (vector + graph + full-text in one); design the
chunk/embedding tables so vectors could be exported to pgvector later. Don't run all
three — that's three systems to keep consistent for no benefit at this scale.

### What goes in the graph (why graphrag helps here)

```
(Saying)-[:USES_MOVE]->(ReasoningMove M01..M18)
(Saying)-[:SPOKEN_TO]->(Audience)
(Saying)-[:AT]->(Location)
(Saying)-[:MENTIONS]->(Concept)
(Saying)-[:PARALLELS]->(Saying)        // synoptic parallels
```

Vector search finds *semantically* similar sayings; the graph finds *structurally*
related ones (same move, same audience, synoptic parallels) that pure similarity
misses — which produces richer, less repetitive context and powers the mind-map view.

---

## 3. Instruction tuning — do it, but not the way the phrase implies

**Verdict: do NOT run a separate, general instruction-tuning stage. It is unnecessary,
costly, and risks catastrophic forgetting on a corpus this small.** Instead:

1. **Start from an already instruction-tuned (`-Instruct`) checkpoint.** You inherit
   robust instruction-following for free — that's exactly what the Instruct variant
   already did at scale. Re-doing it yourself is redundant.
2. **Blend a *small* set of in-domain conversational examples into the same LoRA SFT
   mix** — realistic user questions ("What did Jesus say about worry?") paired with
   grounded, cited answers. This is *task alignment*, not general instruction tuning:
   it teaches the twin to follow the *kinds* of instructions your users will give, in
   the twin's register.
3. **Mix in a little general instruction data** alongside the domain data to mitigate
   forgetting. The research is consistent: PEFT/LoRA forgets less than full FT, and
   interleaving general data reduces drift further. Starting from an Instruct base is
   itself the single biggest forgetting-mitigation lever.

Why the nuance matters: much of what looks like "forgetting" is *task/format drift* —
the model stops interpreting prompts as the intended task. A full instruction-tuning
pass on a few hundred domain rows is the fastest way to *cause* that drift. A light
LoRA on an Instruct base, with a small blended instruction set, gets you
instruction-following *and* the twin's voice without the risk.

> This section was deliberately run through a sycophancy check. An earlier draft that
> said "you should absolutely do instruction tuning" was flagged for stating a
> high-confidence conclusion with no trade-offs — the corrected position is the
> nuanced one above.

---

## 4. Rust stack

| Layer | Crate / tool | Role |
|---|---|---|
| Inference | **mistral.rs** (on Candle) | OpenAI-compatible server; GGUF + SafeTensors; **LoRA / X-LoRA** with weight merging; Metal + CUDA + CPU; in-situ quantization |
| Inference (alt) | **Candle** directly, or **llama-cpp-2** / **Kalosm** | Lower-level control, or thin llama.cpp bindings |
| Edge / phone | **llama.cpp** (GGUF `Q4_K_M`) or **MLX** (Apple) / **LlamaEdge** (Wasm) | On-device inference of the phone tier (Gemma 4 E2B / Llama 3.2 3B) |
| Store | **surrealdb** crate (embedded) | Vector + graph + full-text + RRF in one query |
| Embeddings | **fastembed-rs**, or ONNX via **ort** | Local embedding generation; or a remote embedding API |
| Data pipeline | Rust port of `build_training_jsonl.py` | xlsx → `sft_style.jsonl` + `rag_corpus.jsonl` + `eval_heldout.jsonl` |
| Training | **Unsloth + QLoRA** (Python, offline) → export GGUF/adapter | Training is still best in the Python/Unsloth toolchain; Rust consumes the resulting adapter/GGUF |

**Note on training in Rust:** Rust's *inference* story is strong (mistral.rs / Candle),
but the *fine-tuning* toolchain (Unsloth, PEFT, TRL) is still Python. The pragmatic
split is **train in Python/Unsloth, serve in Rust** — train the LoRA, merge or load it
in mistral.rs, and run the whole serving + RAG + graph app in Rust. Candle can train in
principle, but you'd be reimplementing what Unsloth already optimizes.

---

## 5. Build order

1. **Port the data pipeline** to Rust: read the annotated `jesus_full_red_letter.xlsx`,
   emit `sft_style.jsonl`, `rag_corpus.jsonl`, `eval_heldout.jsonl`.
2. **Stand up SurrealDB embedded**: load `rag_corpus.jsonl`, build the vector index and
   the `USES_MOVE` / `SPOKEN_TO` / `MENTIONS` / `PARALLELS` graph.
3. **Ship a RAG-first, base-model version** (mistral.rs serving an Instruct GGUF + the
   coverage guardrail). This alone is useful and safe — no fine-tuning yet.
4. **Train the style LoRA** (Python/Unsloth, QLoRA, Gemma 4 E4B or Qwen3-4B Instruct
   base, thinking mode off) with the blended in-domain instruction examples; export and
   load into mistral.rs.
5. **Evaluate** on `eval_heldout.jsonl`, faceted by reasoning move: grounding /
   no-fabrication first, then style fidelity, citation integrity, refusal behavior.
6. **Keep the LoRA only if** it improves style-by-move *without* hurting grounding.

---

## 6. The one rule that doesn't change

The LoRA only ever learns to transform a **real, cited line** into modern voice — it is
never asked to generate teachings. Retrieval owns truth; the adapter owns voice; the
coverage gate refuses out-of-corpus questions. That structure is what keeps the twin
from inventing scripture, in Rust or anywhere else.

---

## Sources

- On-Device LLMs: State of the Union 2026 — https://v-chandra.github.io/on-device-llms
- Best Small Language Models 2026 — https://localaimaster.com/blog/small-language-models-guide-2026
- Fine-Tune Local LLMs 2026 (SitePoint) — https://www.sitepoint.com/fine-tune-local-llms-2026
- Fine-Tune LLMs with LoRA and QLoRA: 2026 Guide (DEV) — https://dev.to/jangwook_kim_e31e7291ad98/fine-tune-llms-with-lora-and-qlora-2026-guide-33lf
- How to Fine-Tune LLMs in 2026 (Spheron) — https://www.spheron.network/blog/how-to-fine-tune-llm-2026
- Fine-tuning 7B on a consumer GPU (CraftRigs) — https://craftrigs.com/guides/fine-tuning-7b-llm-consumer-gpu-unsloth-lora
- SurrealDB — Graph RAG needs a database that does everything — https://surrealdb.com/blog/graph-rag-does-not-need-a-graph-database-it-needs-a-database-that-does-everything
- SurrealDB — Knowledge Graph RAG query patterns — https://surrealdb.com/blog/knowledge-graph-rag-two-query-patterns-for-smarter-ai-agents
- Top 15 Vector Databases in 2026 — https://medium.com/@pratik-rupareliya/top-15-vector-databases-in-2026-a-production-decision-guide-from-100-enterprise-deployments-dd58a04f51a5
- mistral.rs (Rust inference, Candle) — https://docs.clore.ai/guides/language-models/mistral-rs
- Rust Ecosystem for AI & LLMs — https://hackmd.io/@Hamze/Hy5LiRV1gg
- Avoiding catastrophic forgetting in LLM post-training — https://medium.com/@baicenxiao/avoiding-amnesia-some-practical-guides-to-mitigate-catastrophic-forgetting-in-llms-post-training-6a23e4f064cb
- What Is Instruction Tuning? (IBM) — https://www.ibm.com/think/topics/instruction-tuning
- Gemma 4 model card (Google AI for Developers) — https://ai.google.dev/gemma/docs/core/model_card_4
- Gemma 4: most capable open models (Google Blog) — https://blog.google/innovation-and-ai/technology/developers-tools/gemma-4
- Welcome Gemma 4 (Hugging Face) — https://huggingface.co/blog/gemma4
- Gemma 4 — Google DeepMind — https://deepmind.google/models/gemma/gemma-4
- Tool Calling with Gemma 4 and Python — https://machinelearningmastery.com/how-to-implement-tool-calling-with-gemma-4-and-python
