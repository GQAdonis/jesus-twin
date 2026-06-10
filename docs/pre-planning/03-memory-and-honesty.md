# 03 — Episodic Memory and the Visible Honesty Surface (Gaps #5, #6)

> These two gaps share a theme: the difference between *having* a capability and the user
> *experiencing* it. Memory makes the mentor relationship real across sessions; the
> honesty surface makes the project's actual differentiator visible.

## 1. Theory: why a mentor needs episodic memory

A mentor who forgets every previous conversation is a search engine with good manners.
The persona-simulation literature's strongest non-fine-tuning idea (the "Method D" family
from the 2026-06-09 assessment — generative agents, Human Simulacra, R-CHAR) is the
**memory stream**: a time-ordered store of observations about the relationship, retrieved
selectively into context when relevant.

The canonical design (Park et al., *Generative Agents*, 2023) has three operations:
- **Record:** after each turn, store salient facts as discrete memory objects.
- **Retrieve:** before each turn, score stored memories by *recency × relevance ×
  importance* and inject the top few into the prompt.
- **Reflect:** periodically synthesize low-level memories into higher-level ones
  ("Sarah has asked about forgiving her brother three times" → "the brother
  relationship is an ongoing weight for Sarah").

**The critical adaptation for this project:** memory is a **fourth surface** with its own
owner, and it must never contaminate the other three. Memories are facts *about the
user and the relationship* — never new content *about Jesus*. A memory may say "user
asked about anxiety on June 3"; it may never say "Jesus thinks the user should change
jobs." Memory feeds the *personalization* of responses; truth still comes only from the
927 cited passages.

## 2. Design: where memory lives and what a record looks like

The embedded SurrealDB store (`jesus-twin-store`) already does vector + BM25 + graph —
exactly what a memory stream needs. Add a `memory` table, isolated from the corpus
tables (separate table = structurally impossible for corpus retrieval to return a memory
as if it were scripture).

**Example memory records (the three kinds, with their fields):**

```jsonc
// Kind 1: observation — atomic fact from one turn
{
  "kind": "observation",
  "session": "s-2026-06-09-a",
  "at": "2026-06-09T14:02:11Z",
  "text": "User is anxious about money after a job loss; asked whether worry helps.",
  "refs_cited_in_reply": ["Matthew 6:25-34"],
  "importance": 7        // 1-10, model-scored at write time (see §3 step 2)
}

// Kind 2: reflection — synthesized across observations (generated weekly or every N turns)
{
  "kind": "reflection",
  "at": "2026-06-16T09:00:00Z",
  "text": "Money anxiety is a recurring thread (3 sessions). User responds better to the counter-question move than to direct comfort.",
  "derived_from": ["mem:abc1", "mem:abc4", "mem:abc9"],
  "importance": 8
}

// Kind 3: preference — explicit user-stated facts (highest precedence, never inferred)
{
  "kind": "preference",
  "at": "2026-06-09T14:10:00Z",
  "text": "User asked to be called Sam and prefers shorter answers.",
  "importance": 9
}
```

**What is forbidden in a memory record:** doctrine, predictions, judgments of the user,
or anything phrased as the mentor's *belief about facts not in the corpus*. The record
stores what happened in the conversation, not conclusions about the world.

## 3. Step-by-step implementation

1. **Schema:** add the `memory` table to the SurrealDB schema with fields above + an
   embedding column (reuse the existing embeddinggemma path — memories are embedded
   exactly like passages, just in a different table). Index: HNSW on embedding + BM25 on
   `text`, mirroring the corpus indexes you already have.
2. **Write path:** in `jesus-twin-core`, after `RunFinished`, spawn a post-turn step that
   (a) extracts 0–2 candidate observations from the turn (a single model call with a
   strict JSON schema; instruct it to return an empty list when nothing is salient),
   (b) scores importance 1–10, (c) writes records. Salience instruction: "facts about the
   user's situation, recurring topics, or stated preferences — never theological content."
3. **Read path:** in the orchestrator, *before* retrieval, fetch top-K (K=3) memories by
   `0.5·relevance + 0.3·recency + 0.2·importance` (relevance = cosine similarity to the
   user's question; recency = exponential decay, e.g. half-life 14 days). Inject them
   into the prompt as a *labeled* block, system-side, NOT mixed with scripture:
   ```
   [What you remember about this person from earlier conversations:]
   - Money anxiety is a recurring thread; the counter-question lands better than comfort.
   ```
   Placement: after the system contract, before the user turn. The scripture block keeps
   its existing position and label (00-theory §5) — two differently-labeled provenance
   blocks, never merged.
4. **Reflection job:** a `jesus-twin-cli` subcommand (`memory reflect`) run manually or
   on a timer: cluster recent observations (same embedding space), synthesize reflections
   with one model call per cluster, write Kind-2 records. Start manual; automate later.
5. **Controls (non-negotiable, from the project's principles):** a `memory` skill exposing
   `list / export / delete` so the user can inspect and erase everything (principle 15:
   human override always exists). Memory is per-user, local, and excluded from any
   telemetry. Deleting a memory also deletes reflections derived from it
   (`derived_from` makes this traceable).
6. **Tests:** (a) a memory written in session 1 is retrieved in session 2 for a related
   question; (b) an *unrelated* question retrieves nothing (relevance floor — set a
   minimum cosine threshold, e.g. 0.35, below which memory stays silent); (c) corpus
   retrieval results never include memory-table records (query-level isolation test);
   (d) delete removes the record and its derived reflections.

## 4. Theory: why the honesty must be *visible* (gap #6)

The assessment's sharpest finding: against frontier prompt-persona apps ("Text With
Jesus" et al.), this project cannot win on raw fluency — it wins on **verifiable
honesty**: every claim cited, attestation graded, refusal instead of confabulation. But
today those signals exist only in the event stream. A user who cannot *see* the
citations experiences only the smaller model's plainer prose — the differentiator is
invisible exactly where it matters, at the screen.

The mechanism already designed for this (`ALIGNMENT_AND_TUNING.md` §4) is the **custom
AG-UI chunks** — additive, namespaced events a UI can render and standard clients safely
ignore: `x-jesus-twin/citation`, `attestation`, `reasoning-move`, `source-text`,
`interpretation-flag`, `mindmap-delta`.

## 5. Step-by-step: surfacing the honesty

1. **Verify emission:** the AG-UI adapter already maps `AgentEvent::Citation` etc.
   Confirm each custom chunk type is actually emitted on a live turn (curl the AG-UI SSE
   endpoint, grep for `x-jesus-twin/`). Anything missing is an adapter mapping task, not
   a core change.
2. **Reference UI (the React app, when built) — minimum honest surface:**
   - **Citations as chips** under each answer (`Mark 12:29-31`), click → expands the
     original WEB text side-by-side with the modern phrasing (`SOURCE_TEXT` chunk).
   - **Attestation badge** per answer: a small marker (e.g., "multiply attested" /
     "single source") from the `ATTESTATION` chunk — with a hover explaining what the
     tier means and that tiering is contested scholarship (link the methodology page).
   - **Refusals styled as in-voice honesty, not errors.** A refusal renders in normal
     message styling with a subtle "outside the recorded corpus" tag — the gate refusing
     IS the product behaving correctly; never render it red.
   - **Reasoning-move tag** (optional, collapsible): "counter-question (M01)" for users
     who want to see the method — the study-aid audience.
3. **Name it in the product copy.** One line under the composer: *"Answers draw only on
   the recorded sayings; everything is cited, and questions outside the record are
   declined."* That sentence is the positioning against every prompt-persona app.
4. **Test:** a screenshot test of one answered question must show: answer, ≥1 citation
   chip, attestation badge. A refusal screenshot must show the in-voice decline with the
   tag. These two screenshots are the definition of done for gap #6.
