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

---

# modern-legs-v1 — reviving the dead modern retrieval legs (2026-06-17)

Two of the four retrieval legs (`text_modern` BM25 + `emb_modern` vector) were dead: `text_modern`
is empty for all 927 sayings until human annotation fills the `Modern Rendering` column. This change
populates them with **doc2query-style machine drafts** — a modern-English rewrite of each saying's
`text_original` — so a modern-phrased query matches lexically (modern BM25) and semantically
(`emb_modern`), improving recall **now** without waiting on annotation.

## The bright line (enforced, not just stated)

Machine text may influence *which* passages retrieve, never *what is said or trained*:
- Drafts live in a **sidecar** (`build/modern_drafts.jsonl`, git-ignored), each `machine_draft: true`,
  and are applied to `saying.text_modern` with a `machine_draft` flag — never written to the xlsx or
  `rag_corpus.jsonl`.
- **Display:** `orchestrator::context_lines` uses `text_original` only — pinned by the
  `display_uses_original_not_modern` regression test.
- **Training:** `build_training_jsonl.py` reads the human `Modern Rendering` xlsx column, never the
  sidecar.
- A human-verified rendering later overwrites the draft and resets `machine_draft = false`
  (row-by-row promotion as Wave-3 annotation proceeds).

## Pipeline (new CLI subcommands)

```bash
# 1. Generate drafts (plain generation, NOT the twin orchestrator). --limit N to sample.
jesus-twin modern-drafts ../build/rag_corpus.jsonl --out ../build/modern_drafts.jsonl
# 2. Apply into a store + re-embed the modern legs (needs --features cuda for the embedder).
jesus-twin apply-modern-drafts ../build/modern_drafts.jsonl --db ./twin.db
# 3. Re-run the gate calibration — live_legs goes 2 -> 4; expect more 2-leg agreement.
jesus-twin gate calibrate --eval-dir ../eval --db ./twin.db --out ../eval/out/gate-calibration.jsonl
```

## Verified on a 10-passage sample

Drafts are clean modern paraphrases, no commentary ("Blessed are the poor in spirit" → "Those who are
humble in spirit are fortunate"; "Man shall not live by bread alone" → "People must live by more than
just food…"). After apply, a query using **draft-only wording** ("humble in spirit are fortunate" —
absent from the original) returns Matthew 5:3 as the top hit, proving the modern BM25 leg is live.

## Handoff — the full corpus run (the GPU step to trigger)

Run steps 1–3 above without `--limit` against the canonical `twin.db`: ~927 plain generations
(multi-hour) then a re-embed and recalibration. Expected effect: covered queries reach `live_legs = 4`
and more reach 2-leg agreement (Tier 1), which should *raise* the gate's grounding confidence; the
recalibration quantifies it and is the regression baseline. (The ~300-row human-annotation milestone
later promotes drafts to verified renderings and triggers another recalibration — a no-op under
leg-agreement if nothing shifted.)

---

# hebrew-bible — the Tanakh as a labeled source tool (2026-06-18)

The Hebrew Bible (JPS 1917) is *his intellectual furniture* — what he quoted, alluded to (*remez*),
and reasoned from (*kal v'homer*). This change makes it a **separate, retrievable corpus**, always
labeled **his source material, never his words** (CLAUDE.md Bible scope).

## Source — public-domain JPS 1917 via Sefaria (NOT the fragile scrape)

The old `ingest_tanakh.py` scraped sacred-texts.com with heuristic URL slugs + fake verse counters
(now 403, and mis-referenced). Rewritten to use the **Sefaria API**, which serves the JPS 1917
**verse-accurate** as a named version. Critical: Sefaria's *default* English is the modern RJPS
(CC-BY-NC, copyrighted), so the script pins the exact public-domain version
`"The Holy Scriptures: A New Translation (JPS 1917)"` (license "Public Domain") via `ven=`. Stdlib
only — no third-party deps. Verified: Genesis + Exodus → 2,743 verse-accurate records ("In the
beginning God created the heaven and the earth.").

## Store — a separate `tanakh` table, never blended

- Schema: `tanakh` table (`ref/text/book/category/translation/emb`) with its own BM25 + HNSW
  indexes — wholly separate from `saying`.
- `SourcePassage` is a distinct type from `Passage`, so the two corpora can't be conflated;
  adapters/CLI label results "source material… NOT his own words".
- `SurrealStore::ingest_tanakh` (+ `embed_tanakh`) and `retrieve_tanakh` (BM25 + vector RRF).
- New CLI: `ingest-tanakh`, `retrieve-tanakh`. Test `ingests_tanakh_as_separate_source_corpus`
  pins the load + the **separation invariant** (Tanakh must not leak into red-letter retrieval).

## Verified on a sample (CPU, BM25)

Ingested the 2,743-verse Genesis+Exodus sample text-only; `retrieve-tanakh "created the heaven and
the earth"` → Genesis 1:1/2:4/2:1; `"bread from heaven manna"` → the Exodus 16 manna verses — each
under the source-material label. The red-letter `retrieve("bread")` returns nothing (separation holds).

## Handoff — the full corpus run (the GPU/network step to trigger)

```bash
python ingest_tanakh.py --out build/tanakh.jsonl                 # ~23k verses from Sefaria (network)
jesus-twin ingest-tanakh ../build/tanakh.jsonl --db ./twin.db    # load + embed (GPU; --features cuda)
jesus-twin retrieve-tanakh "have you not read" --db ./twin.db    # spot-check
```

## Deferred (noted, not in this change)

Surfacing the Tanakh as a "his source material" block inside `ask`/`serve` answers (an orchestrator
addition that runs `retrieve_tanakh` alongside the red-letter retrieval and emits it as a distinct,
labeled context block / AG-UI chunk) — the store + retrieval primitives are in place; the
orchestrator wiring + the M09/remez cross-reference graph edges are a follow-up.

---

# principle-index-v1 + principle-tier — the general-mentor unlock (Wave 2, 2026-06-19)

These two coupled changes turn Tier-2 from "decline what the passages don't cover" into
**principle-bridging**: name the governing principle the cited passages establish and speak to how
it bears on an adjacent life question — then say where the record stops.

## principle-index-v1 — life-domain + principle facets

- A fixed ~20-domain **taxonomy** (`TAXONOMY` in the CLI): money/provision, fear/anxiety, grief,
  marriage/divorce, parenting, conflict/forgiveness, ambition/status, honesty, illness,
  purpose/calling, enemies, doubt, prayer, wealth/generosity, judgment-of-others, work, power,
  loneliness, temptation, death.
- New `saying` facets `domains` + `principles` (+ `machine_tagged` flag) and on `Passage`. **Same
  bright line as modern-legs:** machine tags are retrieval metadata only — never displayed
  (`context_lines` uses `text_original`) or trained (SFT reads the xlsx). A wrong tag costs at most
  a retrieval miss, never fabricated content.
- Pipeline (mirrors modern-legs): `principle-tag` (plain LLM generation → `DOMAINS:`/`PRINCIPLE:`,
  parsed leniently into the taxonomy) → sidecar `build/principle_tags.jsonl`; `apply-principle-tags`
  loads facets into the store (no re-embed — facets don't change vectors). Parser unit-tested.

## principle-tier — Tier-2 principle-bridging

- No gate change needed: the orchestrator already has `set.passages[].principles`. On a
  `LowConfidence` turn it `collect_principles(&set)` and, if any, assembles
  `assemble_context_principle_tier` (the `PRINCIPLE_BRIDGING_HEAD` + the principles) instead of the
  plain hedge — a per-turn context injection; **SYSTEM_PROMPT untouched**. The principles also ride
  on the `x-jesus-twin/low-confidence` chunk for the honesty surface.
- Falls back to the plain low-confidence hedge when no principles are tagged (so it's a safe no-op
  until tagging runs). Prompt + orchestrator integration tests pin both paths.

## Handoff — the full tagging run (GPU)

```bash
jesus-twin principle-tag ../build/rag_corpus.jsonl --out ../build/principle_tags.jsonl  # ~927 gens
jesus-twin apply-principle-tags ../build/principle_tags.jsonl --db ./twin.db
# then Tier-2 answers bridge via the tagged principles; re-run gate calibrate as regression.
```

## Deferred follow-up

The plan's **theme-expansion retrieval boost** (embed question → nearest domain → boost
domain-tagged passages as a 5th RRF leg) is not in this v1 — the facets + bridging land first;
boosting recall via the domain leg is a contained follow-up. Human review later promotes tags
(`machine_tagged = false`).

---

# episodic-memory — the fourth surface (Wave 2, 2026-06-19)

A relationship memory that records facts about the **user**, never about Jesus (pre-planning 03).
The load-bearing invariant: **memory must never contaminate the other three surfaces** — a memory
may say "user asked about anxiety on June 3"; it may never become new content about Jesus.

## Isolation, structurally enforced

- A separate `memory` SurrealDB table (kind / scope / text / importance / at / refs). Corpus
  retrieval (`Store::retrieve`) only ever reads `saying`/`tanakh` — it is **structurally impossible**
  for it to return a memory. The `memory_is_isolated_and_scoped` test pins this.
- `scope` keys one relationship (`Session::memory_scope()` = user id if known, else session id).
  Every memory read/write filters on it — **memories never cross relationships** (tested A vs B).
- New `Store` methods (`record_memory` / `retrieve_memories` / `list_memories` / `delete_memory`)
  with **default no-ops**, so only SurrealDB implements them and test doubles are unaffected.

## Wiring

- `Session` gains `user_id: Option<Uuid>` + `with_user()` (backward-compatible; `Session::new`
  unchanged). Anonymous sessions scope memory to the single conversation.
- Orchestrator: **pre-turn** it recalls the top-3 salient memories (importance, then recency) and
  injects them as a `[What you remember about this person…]` block BEFORE the grounding block — a
  per-turn context injection, **SYSTEM_PROMPT untouched**. **Post-turn** (non-refused) it records a
  deterministic observation ("Asked: …" + the verses that grounded the reply). Both are **non-fatal**.
- CLI `memory list <scope>` / `memory delete <id>` — the human inspect / override control
  (CLAUDE.md principle 15).

## v1 scope (deferred)

Deterministic observations (the user's question + cited refs); **reflection synthesis** (LLM
rollup across observations) and **relevance-ranked recall** (vector/BM25 over memory, vs. the
current importance×recency) are documented follow-ups. The `preference` kind exists in the schema
but is not yet auto-extracted.

---

# gospel-context-kb — the third labeled corpus: what he DID (Wave 2, 2026-06-20)

The non-red-letter Gospel narrative — his deeds, settings, and the dialogue around the sayings —
as a **third labeled corpus**: "what the record shows he did," never his words (the red-letter
`saying` corpus). Gives Tier-2 answers access to *example by deed* with the same citation discipline.

## Source — the complement of the red-letter extractor

`extract_gospel_narrative.py` reuses the WEB USFX (eBible.org, public domain) the red-letter
extractor downloads, but keeps the **complement**: each Gospel verse's text with the `<wj>` (his
words) spans AND the editorial apparatus (`<f>` footnotes, `<x>` cross-refs) removed. A verse is
emitted only when its remaining narrative text is substantial (≥25 chars), so a bare speech tag
("He said to them,") is dropped while a deed ("Being moved with compassion, he stretched out his
hand and touched him") is kept. Verified: **2,182 clean narrative passages** across the four
Gospels; footnotes gone, no his-words leak, pure sayings (e.g. Mark 1:17) correctly absent.

## Store — a separate `gospel_narrative` table

- Schema mirrors `tanakh` (ref/text/book + BM25 + HNSW), plus `attestation` + `witnesses`.
- `NarrativePassage` is a distinct type — adapters/CLI label results "what the record shows he
  did… NOT his own words." `SurrealStore::ingest_gospel_narrative` / `retrieve_gospel_narrative`
  (BM25 + vector RRF, via the generic `rank_table_*` helpers). CLI `ingest-gospel-narrative` /
  `retrieve-gospel-narrative`. Test `ingests_gospel_narrative_as_separate_corpus` pins the load +
  the separation invariant (narrative must not leak into red-letter retrieval).
- Verified on the full 2,182-passage extraction (CPU/BM25): "touched the leper and healed" →
  Luke 22:51 / Luke 6:19 / Mark 3:10 (healing deeds), labeled + attestation-flagged.

## Honest deferral — automated attestation v1 is BLOCKED

The plan's "multiply-vs-single attestation computed mechanically from synoptic-parallel counts"
**cannot be done yet**: the corpus has **no synoptic-parallel data** (the `parallels` graph is
unpopulated — ingest.rs has always noted this). So `attestation` defaults to `single`; computing
true multiply-attestation needs a synoptic-parallel mapping (pericope alignment across the
Gospels), which is its own data task. Documented as the follow-up; the corpus + retrieval + label
land now, attestation-flagged and ready to populate.

## Handoff — the full run (network + GPU)

```bash
python extract_gospel_narrative.py --out build/gospel_narrative.jsonl   # ~2.2k passages (network)
jesus-twin ingest-gospel-narrative ../build/gospel_narrative.jsonl --db ./twin.db   # load + embed (GPU)
jesus-twin retrieve-gospel-narrative "he wept at the tomb" --db ./twin.db
```
Orchestrator wiring (surfacing a labeled "what the record shows he did" block in `ask`/`serve`
answers) is the same follow-up flagged for hebrew-bible — the store + retrieval primitives are in
place.

# Orchestrator wiring — the two source/narrative blocks are now live

The hebrew-bible and gospel-context-kb deferrals above ("surface a labeled block in the answer")
are **closed**. Each turn now retrieves, with the same user query, two SUPPLEMENTARY labeled
corpora alongside the red-letter `set` and injects them as DISTINCT context blocks:

- **His source material** (Tanakh) → `prompt::SOURCE_INSTRUCTION` block ("the Hebrew scriptures you
  drew on … not your own teaching") + an additive `x-jesus-twin/source-text` AG-UI chunk
  (ref/text/category).
- **His deeds** (Gospel narrative) → `prompt::NARRATIVE_INSTRUCTION` block ("what the record shows
  you did … deeds, not your words") + an `x-jesus-twin/narrative-context` chunk
  (ref/text/attestation/witnesses).

Context order is ascending attention: memory → source → narrative → **grounding (his own words)
last**, the high-attention end-of-prompt position. `SUPP_LIMIT = 2` each, so they contextualize
the answer without ever dominating the red-letter truth the model paraphrases. Retrieval is
non-fatal (`unwrap_or_default`, like memory recall) and happens after the coverage gate, so a
refused turn does no extra work.

## The bug this surfaced — `Arc<SurrealStore>` silently no-op'd the trait defaults

The served orchestrator holds the store behind an `Arc`. The blanket `impl<S: Store> Store for
Arc<S>` only forwarded four methods; every method with a default impl (the memory quartet, and now
the two `retrieve_*` corpora) fell through to the **default no-op** instead of the inner store.
That means **episodic-memory was already a silent no-op through `Arc` in the served path** (the
direct-`SurrealStore` tests passed, hiding it). Fixed by forwarding all of them through `Arc`. The
two corpus retrievers were also promoted from inherent methods to `Store` trait methods so they
forward at all (the orchestrator is generic over `S: Store`).

## Verification

`source_and_narrative_corpora_wire_as_distinct_labeled_blocks` (a capturing engine records the
generation context) asserts: both AG-UI chunks emit with their distinguishing facets; both labeled
blocks reach the context; the red-letter grounding block is still present; and grounding sits AFTER
source + narrative. Full `jesus-twin-core` suite (10 unit + 6 integration) green; `jesus-twin-store`
(7) green; `cargo clippy --workspace` clean.
