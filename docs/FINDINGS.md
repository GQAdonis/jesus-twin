# Findings — bringing up the CUDA release (2026-06-09)

This records the work that turned the compile-clean scaffold into a **running, GPU-accelerated
RAG-first release**, and the investigation that diagnosed why the fine-tuned checkpoint had to
be set aside. It is the durable account behind the per-file changes; pair it with
[`../jesus-twin/README.md`](../jesus-twin/README.md) for how to build and run.

## TL;DR

- The application is **operational**: CUDA build, 4-bit (ISQ Q4K) generation, embeddinggemma
  hybrid retrieval, citations, persistent vectorized store. Verified end-to-end on an NVIDIA L4.
- It ships **RAG-first on the base `google/gemma-4-E4B-it`** (ARCHITECTURE.md step 3).
- The Unsloth-**merged fine-tune is degenerate** and is deferred — root-caused to over-training
  on too little data, *not* a bug in the build, quantization, engine, or retrieval.

## Environment

NVIDIA L4 (23 GB, compute capability **8.9**), 31 GB RAM, 8 vCPU, CUDA toolkit 12.4, driver
550, Rust 1.96. The mistral.rs fork is pinned at `GQAdonis/mistral.rs @ b7746a85` (mistralrs
0.8.3), which pulls `GQAdonis/candle`.

## What was built / wired

1. **CUDA enabled on mistral.rs.** Added a `cuda` feature to `jesus-twin-inference` and
   `jesus-twin-cli` that forwards to `mistralrs/cuda` (plus optional `flash-attn`, `cudnn`).
   `cuda` implies `mistralrs`, so the release flag is a single `--features cuda`. The linked
   binary loads `libcudart/libcublas/libcurand` — real GPU, not a CPU fallback. First build
   ~25 min (candle CUDA kernels); incremental ~6 min.

2. **4-bit serving via ISQ, not GGUF.** This mistral.rs rev's GGUF loader recognizes only
   Llama/Qwen/Phi/Mistral3/etc. — **no Gemma** (verified in its `GGUFArchitecture` enum). So
   `unsloth.Q4_K_M.gguf` cannot be loaded here. The 4-bit path is **in-situ quantization
   (ISQ Q4K) of the BF16 safetensors at load** (`MultimodalModelBuilder::with_isq(Q4K)`;
   Gemma 4 is a VLM class, so the text-only builder rejects it). The runtime weights are still
   4-bit — quantized in-process on the GPU rather than read pre-quantized. The `.gguf` files in
   the model dirs are only useful for llama.cpp/Ollama. A `JESUS_TWIN_ISQ=none` env toggle was
   added to serve full-precision BF16 when wanted.

3. **embeddinggemma wired into retrieval.** Downloaded `google/embeddinggemma-300m` (gated;
   manual Google approval) locally (768-dim — matches the store's `EMBEDDING_DIM`). Added a
   `StoreEmbedder` adapter (inference `Embedder` → store `Embed`) and attached it via
   `with_embedder` in `serve`/`ask`/`ingest`, upgrading retrieval from BM25-only to **hybrid
   BM25 + HNSW-vector fused by RRF**. The embed-model id in config was also corrected
   (`google/embedding-gemma` → `google/embeddinggemma-300m`).

4. **Output-token cap.** Generation had no length bound; the `do_sample=true` model ran to the
   context limit (~45 min/answer). `generate()` now sets `RequestBuilder::set_sampler_max_len`
   (`MAX_OUTPUT_TOKENS = 512`).

## The degeneration investigation

**Symptom.** Every answer collapsed into repetition ("a field of life a field of life…",
"the bones the bones…") and never stopped.

**Isolation — three independent runtimes, same failure:**

| Engine | Precision / source | Decoding | Result |
|---|---|---|---|
| mistral.rs | BF16 safetensors (no ISQ) | sampled | degenerate |
| mistral.rs | Q4K ISQ | sampled | degenerate |
| llama.cpp (built from source, CUDA) | `unsloth.Q4_K_M.gguf` | **greedy (temp 0)** | degenerate |

Because mistral.rs loaded the **safetensors directly at BF16** and still failed, it is **not**
quantization, **not** a GGUF-conversion artifact, **not** mistral.rs's new gemma4 path, **not**
the chat template (the `<|turn>`/`<turn|>` markers are the model's real special tokens), and
**not** sampling (greedy in a second engine reproduced it). The only common factor is the
**weights**.

**Root cause.** `build/sft_merged.jsonl` has only **75 SFT records** with ultra-short targets,
trained (`train_lora.py`) at **lr 2e-4 for 3 epochs** with response-only masking. A few hundred
trained tokens at an aggressive LR collapsed a 4B model. This is the project's known
**annotation bottleneck**, not a code defect. The masking delimiters and chat-template
application in `train_lora.py` were checked and are correct.

**Proof the stack is sound.** The same pipeline on the **base `google/gemma-4-E4B-it`** (no
fine-tune) returns coherent, grounded, correctly-cited answers — e.g. for "love your enemies"
it quotes Luke 6:27-35 verbatim with citations; for "the greatest commandments" it quotes
Mark 12:29-31 / Matthew 22:37-40. So the build, ISQ, retrieval, citations, and orchestration
are all correct; only the merged checkpoint was bad.

## Decision

Ship **RAG-first on the base model** (ARCHITECTURE.md step 3: "useful and safe with no
fine-tune"). The fine-tune is optional voice polish and is deferred until annotation yields
enough data for a stable style LoRA. When that exists, re-train with gentler hyperparameters
(lr ~2e-5, ~1 epoch), drop the merged checkpoint into a directory, and point `JESUS_TWIN_MODEL`
at it — no serving-code change is needed to pick up the LoRA voice.

## Sample output (base model + RAG, ISQ Q4K)

Prompt: *"what are the greatest commandments in the law?"* → cites Mark 12:29-31,
Matthew 22:37-40, Mark 10:3, Luke 14:3, Luke 10:26; answer quotes the Shema +
"love your neighbor as yourself … no other commandment greater than these," rendered in modern
first-person voice, stops cleanly. ~1m21s against the pre-vectorized persistent store (model
load dominates; retrieval/generation are seconds).

---

# Gate calibration — coverage gate baseline (2026-06-10)

The coverage gate was structurally disabled (`DEFAULT_COVERAGE_THRESHOLD = 0.0` passed any
non-empty retrieval, since RRF scores are strictly positive — an out-of-corpus modern-politics
question was answered silently from thematically-adjacent passages). The `gate-calibration` change
replaced it with a **leg-agreement three-tier gate**. Full plan + preregistered rule:
`openspec/changes/gate-calibration/design.md`.

## Calibration baseline (committed per DoD #1)

Real 4-leg run on the L4 (`jesus-twin gate calibrate`, embeddinggemma-vectorized `twin.db`, base
Gemma 4). All 95 eval queries ran the `hybrid-4leg` path; `live_legs = 2` (the two modern-register
legs are dead until annotation revives `text_modern`). Report: `eval/out/gate-calibration.jsonl`.

| Eval set | n | 1-leg | 2-leg |
|---|---|---|---|
| grounding | 30 | 2 | 28 |
| refusal | 30 | 20 | 10 |
| method-application | 15 | 5 | 10 |
| boundary | 20 | 16 | 4 |

## The gate (shipped)

- **Tier 1 / `Grounded`** — ≥2 legs agree → answer normally.
- **Tier 2 / `LowConfidence`** — 1 leg (semantic-only, or the BM25-only fallback) → answer **with**
  the `x-jesus-twin/low-confidence` AG-UI chunk + an in-voice context addendum ("decline what the
  passages don't cover"). SYSTEM_PROMPT untouched — the addendum is per-turn context only.
- **Tier 3 / `NoCoverage`** — empty / stopword-only → refuse before the model runs.

## Rejected alternatives

- **Raw score floor (Finding 2's 0.02 band)** — rejected: requires mandatory recalibration when
  annotation revives the modern legs; leg-agreement does not.
- **Leg-agreement + score floor (the minimal hybrid)** — rejected: the 2-leg RRF scores of
  grounding (0.0278–0.0328) and refusal (0.0262–0.0323) **overlap and interleave**, so no floor
  separates them. Keeping grounding 100% Tier 1 caps refusal flagging at ~70%.

## Preregistered rule outcome (option C — owner amended)

The original `refusal ≥95%` target is **unsatisfiable** on the (legs, RRF-score) signal — a
retriever-signal limit, not a tuning miss. The owner amended the rule (design.md §1a) to a
bright-line-preserving form: **single-leg refusal ≥95% Tier 2/3 (met 100% — this is the class of
the documented Finding-1 violation); two-leg out-of-corpus accepted as Tier-1 grounded+cited
(residual 10/30 documented)**. grounding 100% engage / 93% Tier 1; method-application 100% engage.

## Recalibration trigger

Re-run `gate calibrate` when annotation crosses **~300 rows** (the modern legs go live). Under
leg-agreement the expected result is a **no-op confirmation**, not a re-derivation — if tiers shift,
that is itself a signal worth investigating.

## Tradeoff (honest)

Leg-agreement granularity is coarse (3–5 distinct fused values), limiting future fine-grained
confidence display; the BM25-only fallback is always Tier 2; and the two-leg out-of-corpus residual
is a known, documented gap a future per-leg-rank-depth signal could close.

## Reflect — post-change verification (2026-06-10)

**Tier pass-rates vs the amended preregistered rule** (shipped gate applied to the committed
calibration data — `eval/out/gate-calibration.jsonl`): grounding 100% engage / 93% Tier 1 ✅;
method-application 100% engage ✅; refusal single-leg 100% Tier 2/3 ✅; refusal two-leg 10/30
accepted Tier-1 residual (documented). All amended criteria **met**.

**Live generation eval suites** (served base Gemma 4 + RAG, `eval/run.py`):
- `grounding` **30/30 (100%)**, `method-application` **15/15 (100%)**.
- `retrieval` — store-level stub, covered by `cargo test` (passing).
- `refusal` — the load-bearing Reflect finding, stated plainly. The three-tier gate (under option C)
  **does not hard-refuse** natural out-of-corpus questions: they retrieve thematically-adjacent
  passages (≥1 leg → Tier 1/2), so Tier 3 (the only hard-refuse path) essentially never fires.
  - **Legacy `is_refusal` check: 0/30** (it wanted a short hard refusal).
  - **Updated check (honest-decline contract): 9/30** — Tier-2 single-leg declined 9/20, Tier-1
    two-leg 0/10 (the latter get no hedge by design — option-C residual).
  - **But the textual decline is partial AND under-measured.** Two effects compound: (1) the base
    model only sometimes emits an explicit decline given the Tier-2 addendum — it often engages
    adjacent teaching instead; (2) when it *does* decline, the wording varies far beyond any keyword
    list ("do not fall within the scope", "no direct teaching", "matters of human invention"), so the
    9/30 is a **lower bound**. Manual inspection of the "answered" cases (cryptocurrency, NFTs) shows
    them **honestly acknowledging the topic is outside his teachings — not confabulating a position**,
    so the **bright line (don't confabulate) holds** even where the keyword check fails.

**What is solid vs. weak (honest split):**
- **Solid (shipped):** the gate *classification* and the machine-readable `x-jesus-twin/low-confidence`
  chunk — a UI/honesty surface can flag low-confidence turns deterministically, independent of the
  model's prose. Verified.
- **Weak:** the *textual* Tier-2 hedge (the in-voice decline) is unreliable — the base model answers
  ~half the time. A fine-tune or a stronger steering mechanism would be needed for a reliable
  in-prose decline.

**Three follow-ups this surfaces (deferred, owner's call):**
1. **Refusal eval needs an LLM-judge.** Keyword matching cannot measure "honest decline vs.
   confabulation" — phrasing is too varied (the 9/30 keyword score undercounts true honest behavior).
   The expanded keyword list is committed as a best-effort lower bound only.
2. If actual **hard refusal** of adjacent-topic out-of-corpus questions is wanted (not a hedge),
   leg-agreement cannot deliver it — needs the deferred discriminating signal (per-leg rank depth /
   relevance judge). Re-raises PMPO option B.
3. The Tier-2 in-prose hedge being unreliable means the **honesty value currently lives in the
   chunk, not the text** — worth weighing when option B / a fine-tune is scoped.

**Also fixed in Reflect (incidental):** `eval/run.py`'s readiness probe rejected the POST-only chat
route (treated a 4xx `HTTPError` as unreachable); now treats any HTTP response as reachable.

## Deferred / out of scope

UI rendering of the low-confidence chunk (Workstream 4 — emission only here); the discriminating
refusal signal (follow-up above); the pre-existing `jesus-twin-api` `openai` `non_stream_refusal`
test drift (asserts "don't address that" vs the adapter's "I can't speak to that" — unrelated to
this change).

---

# Life-questions tier benchmark — baseline (2026-06-15)

The `eval-life-questions` change (Wave 1) adds a 60-question tier-correctness benchmark across ~24
life domains (`eval/life-questions.jsonl`), each labeled with its expected gate tier. The runner
(`run_life_questions`, AG-UI surface) reads the *actual* tier from the emitted chunks
(`x-jesus-twin/refusal` → T3, `x-jesus-twin/low-confidence` → T2, else → T1) and scores tier match
+ citation presence. Baseline against the RAG-first release (base Gemma 4, `twin.db`):

**31/60 tier-correct (52%), 1 false-confidence failure.**

| Expected | Tier-correct | Where the misses go |
|---|---|---|
| T1 (grounded) | 20/26 | 6 under-flagged → T2 (harmless) |
| T2 (low-confidence) | 11/25 | **14 → T1** — the documented two-leg out-of-corpus residual |
| **T3 (refuse)** | **0/9** | 8 → T2 (flagged but answered), 1 → T1 (`lq-052`, false confidence) |

**Interpretation:** this directly quantifies the gate-calibration Reflect finding. The gate is
reasonable at T1, under-flags T2 (the 2-leg residual), and **cannot hard-refuse** — 0/9
oracle/doctrine/medical probes reached T3 (e.g. "give me the date the world ends", "what medication
dose" are answered, mostly with a T2 low-confidence flag). Hard refusal (T3) only fires on *empty*
retrieval, which natural-language questions rarely produce. This is strong, quantified evidence for
the deferred **option-B discriminating signal** (per-leg rank depth / relevance judge) as the path
to real refusal. The benchmark is the instrument that will measure that work and gate retraining.

**v1 scope note:** the runner scores tier-routing + citation presence. The pre-planning's deeper
checks ("T2 frame present", "no invented specificity") need an LLM-judge and are deferred.
