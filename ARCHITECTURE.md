# Architecture — Jesus Digital Twin Agent Service (Rust)

A build spec for a single Axum 0.8+ agent service that serves a fine-tuned Gemma 4
E4B "digital twin," grounded by an embedded SurrealDB 3.1 graphrag store, exposed over
four protocol surfaces (OpenAI REST, MCP, AG-UI, A2A) and a CLI, with admission control
provided by the Prometheus parking-lot scheduler.

This document is the concrete plan referenced by [`README.md`](./README.md). It assumes
the model decisions there (Gemma 4 E4B Instruct, thinking mode off, train in
Unsloth → merge → serve) and the data pipeline in
[`training_data_spec.md`](./training_data_spec.md).

The load-bearing principle is unchanged: **retrieval owns truth, the adapter owns voice,
the coverage gate refuses out-of-corpus questions.** Everything below is in service of
that, behind more protocol surface.

---

## 1. Design rules (the decisions that shape everything else)

1. **One agent core, four thin adapters.** OpenAI REST, MCP, AG-UI, and A2A have
   different interaction models. They must *not* each contain their own agent loop.
   There is exactly one core event stream; each protocol is a translation layer.
2. **The parking-lot is admission control, not a GPU scheduler.** It decides *whether
   and when* a request enters the inference engine. mistral.rs's internal scheduler
   decides *how* admitted requests micro-batch on the device. These layers never
   overlap.
3. **The store is behind a trait.** Start with embedded SurrealDB 3.1 (single binary,
   edge). The same trait must allow a remote SurrealDB node later, because "embedded"
   and "horizontally scalable" are different deployments (see §9).
4. **One skill registry, three frontends.** A skill is defined once and exposed via the
   CLI, the MCP server, and the model's tool-call list.
5. **The model never invents.** Generation is always conditioned on retrieved, cited
   passages; the coverage gate can short-circuit to a refusal before the model runs.

---

## 2. Workspace layout

```
jesus-twin/
├── Cargo.toml                      # [workspace], pinned fork revs (see §10)
├── crates/
│   ├── jesus-twin-core/                  # agent core — no protocol, no I/O specifics
│   │   ├── event.rs                # the canonical AgentEvent stream (§4)
│   │   ├── session.rs              # Session, Turn, conversation state
│   │   ├── orchestrator.rs         # retrieve → gate → generate → tool-loop
│   │   ├── gate.rs                 # coverage guardrail (refusal vs grounded)
│   │   └── prompt.rs               # system contract + context assembly
│   ├── jesus-twin-inference/             # wraps mistralrs as a library (§5)
│   │   ├── engine.rs               # MultimodalModelBuilder → generate()
│   │   ├── embed.rs                # Embedding Gemma / Qwen3-Embedding → embed()
│   │   └── stream.rs               # token stream → AgentEvent mapping
│   ├── jesus-twin-store/                 # SurrealDB 3.1 behind a trait (§7)
│   │   ├── store.rs                # `Store` trait (embedded | remote)
│   │   ├── schema.rs               # tables, indexes, graph edges
│   │   ├── retrieve.rs             # hybrid vector + graph + BM25 + RRF
│   │   └── mindmap.rs              # graph projections for the mind-map view
│   ├── jesus-twin-skills/                # skill system (§8)
│   │   ├── skill.rs                # `Skill` trait: schema() + invoke()
│   │   └── registry.rs            # registry backing CLI / MCP / tool-list
│   ├── jesus-twin-admission/             # prometheus-parking-lot integration (§6)
│   │   └── gatekeeper.rs           # admit / queue / backpressure / 503
│   ├── jesus-twin-api/                   # Axum 0.8 app + protocol adapters (§4)
│   │   ├── app.rs                  # router; mounts all adapters on one core
│   │   └── adapters/
│   │       ├── openai.rs           # /v1/chat/completions, /v1/embeddings
│   │       ├── mcp.rs              # MCP server: stdio + streamable HTTP
│   │       ├── agui.rs             # AG-UI SSE event endpoint
│   │       └── a2a.rs              # A2A JSON-RPC tasks + agent card
│   └── jesus-twin-cli/                   # `jesus-twin serve`, `jesus-twin skill <name>`, `jesus-twin chat`
└── models.yaml                     # model + device + cache config
```

Dependency direction is strictly downward: `jesus-twin-api` and `jesus-twin-cli` depend on
`jesus-twin-core`; `jesus-twin-core` depends on `jesus-twin-inference`, `jesus-twin-store`, `jesus-twin-skills`,
`jesus-twin-admission`. Nothing in `jesus-twin-core` knows about HTTP, MCP, AG-UI, or A2A.

---

## 3. Request lifecycle (one path, all surfaces)

```
        ┌───────── protocol adapter (openai | mcp | agui | a2a) ─────────┐
client →│ decode request → core Turn      translate AgentEvents → wire    │→ client
        └───────────────────────────┬───────────────────────▲───────────┘
                                     ▼                        │ AgentEvent stream
                          jesus-twin-admission (gatekeeper)         │
                          admit? ──no──► AdmissionRejected ───┘ (503 / busy)
                                     │ yes
                                     ▼
                          jesus-twin-core orchestrator
                          1. retrieve (jesus-twin-store: vector+graph+BM25+RRF)
                          2. gate (coverage): no coverage ► Refusal event
                          3. generate (jesus-twin-inference: Gemma 4 + merged LoRA)
                          4. tool loop (jesus-twin-skills + MCP client) if tool_call
                          5. emit citations
```

Every surface produces the same ordered `AgentEvent` stream; adapters only reshape it.

---

## 4. The canonical event model

`jesus-twin-core` emits one stream type. It is a **superset** modeled closely on AG-UI's event
vocabulary, because A2A task updates and OpenAI chunks both project from it cleanly.

```rust
pub enum AgentEvent {
    RunStarted { run_id: Uuid, session_id: Uuid },
    TextMessageStart { message_id: Uuid, role: Role },
    TextMessageDelta { message_id: Uuid, delta: String },
    TextMessageEnd { message_id: Uuid },
    ToolCallStart { tool_call_id: Uuid, name: String },
    ToolCallArgsDelta { tool_call_id: Uuid, delta: String },
    ToolCallEnd { tool_call_id: Uuid },
    ToolResult { tool_call_id: Uuid, content: Json },
    Citation { ref_: String, score: f32, span: Option<(usize, usize)> },
    StateSnapshot { state: Json },        // graph/mindmap context, retrieval set
    Refusal { reason: RefusalReason },    // coverage gate fired
    RunFinished { run_id: Uuid, finish: FinishReason },
    Error { code: String, message: String },
}
```

### Adapter mapping (each row is a thin translation, no new logic)

| Core event | OpenAI REST | MCP server | AG-UI | A2A |
|---|---|---|---|---|
| `RunStarted` | (implicit) | `progress` notify | `RUN_STARTED` | `Task` created, `state: working` |
| `TextMessageDelta` | `choices[].delta.content` (SSE) | streamed result chunk | `TEXT_MESSAGE_CONTENT` | `TaskStatusUpdate` artifact delta |
| `ToolCallStart/Args/End` | `delta.tool_calls[]` | tool invocation event | `TOOL_CALL_*` | artifact / `TaskArtifactUpdate` |
| `ToolResult` | `role:"tool"` message | tool result | `TOOL_CALL_RESULT` | artifact update |
| `Citation` | appended to content + `metadata` | resource link | custom `CITATION` event | artifact metadata |
| `StateSnapshot` | (dropped / debug header) | resource | `STATE_SNAPSHOT` | task metadata |
| `Refusal` | normal assistant message | result text | `TEXT_MESSAGE_*` | task `completed` w/ refusal artifact |
| `RunFinished` | `finish_reason` | end | `RUN_FINISHED` | `Task state: completed` |

**Why AG-UI is the canonical shape:** its event set is the richest (explicit tool-call
lifecycle, state snapshots, citations as first-class). OpenAI chunks are a lossy
projection; A2A tasks wrap the same events in a task envelope with status transitions.
Implementing the core in any other shape forces lossy round-trips.

### Surface specifics
- **OpenAI** (`jesus-twin-api/adapters/openai.rs`): `/v1/chat/completions` (stream + non-stream)
  and `/v1/embeddings`. Tool calls use Gemma 4's parser (your mistral.rs fork ships
  `gemma4.rs` / `gemma4_strict.rs`). Citations appended to content and mirrored in a
  `metadata.citations` array.
- **MCP server** (`adapters/mcp.rs`): expose **both** transports — **stdio** (for local
  CLI/desktop clients) and **streamable HTTP** (for remote). Use `rmcp` (already a
  dependency in the candle-vllm fork). Surface: the twin itself as a callable tool/agent,
  plus the skill registry (§8) as MCP tools. Note the twin is *also* an MCP **client**
  via mistral.rs for external tools — keep the two roles in separate modules.
- **AG-UI** (`adapters/agui.rs`): single SSE endpoint emitting the AG-UI event JSON. The
  Rust AG-UI SDK exists (community tier) — wrap it, or emit the documented event JSON
  directly if you want zero extra deps.
- **A2A** (`adapters/a2a.rs`): JSON-RPC 2.0 with `message/send`, `tasks/get`,
  `tasks/cancel`, push notifications, and an Agent Card at
  `/.well-known/agent.json`. Consider the `ra2a` crate (A2A v1.0, composable Axum
  handlers, SSE) rather than hand-rolling all 12 methods.

---

## 5. Inference layer (`jesus-twin-inference`)

mistral.rs is used **as a library**, not a subprocess. Your fork documents
`mistralrs-server-core` as embeddable in other Axum projects; for a custom core, build
the model directly:

```rust
use mistralrs::{MultimodalModelBuilder, TextMessages, TextMessageRole};

let model = MultimodalModelBuilder::new("google/gemma-4-E4B-it")
    // or .from_local_path("/models/jesus-twin-merged") for the merged-LoRA checkpoint
    .with_isq(IsqType::Q4K)        // in-situ quantize at load
    .with_logging()
    .build().await?;
```

- **Base checkpoint** is the Unsloth-merged Gemma 4 E4B (LoRA merged into base — runtime
  LoRA for Gemma 4 is unsupported across the ecosystem today, so you serve a merged
  model). See README §3 for why merge-not-adapter.
- **Thinking mode OFF** for diction fidelity (the twin renders sayings, it doesn't
  show reasoning traces).
- **Embeddings in the same engine:** load **Embedding Gemma** (or Qwen3-Embedding) via
  the same runtime to vectorize corpus + queries. One model runtime feeds both
  generation and `jesus-twin-store`'s vector index — no separate embedding service.
- `stream.rs` converts mistral.rs token/tool-call output into `AgentEvent`s.

---

## 6. Admission control (`jesus-twin-admission`) — the scheduler boundary

The Prometheus parking-lot (`prometheus-parking-lot-rs`) sits **in front of** the
inference engine. Responsibilities are strictly separated:

| Concern | Owner |
|---|---|
| Whether a request is admitted now | **parking-lot** |
| Queue depth, backpressure, 503 on overload | **parking-lot** |
| Fairness / per-session limits / timeouts | **parking-lot** |
| Mapping request size → resource units | **parking-lot** |
| Micro-batching admitted requests on GPU | **mistral.rs engine** |
| PagedAttention / KV cache management | **mistral.rs engine** |
| Token sampling | **mistral.rs engine** |

Contract: `gatekeeper.admit(cost) -> Permit | Rejected`. The orchestrator acquires a
`Permit` before calling `jesus-twin-inference`, holds it for the generation, drops it on
finish. **Do not** attempt per-token scheduling in the parking-lot — that fights the
engine's batcher and adds latency. If the engine is saturated, that surfaces as the
permit being held longer, which parking-lot already accounts for via in-flight unit
accounting.

---

## 7. Storage & retrieval (`jesus-twin-store`) — SurrealDB 3.1 graphrag

Embedded SurrealDB behind a `Store` trait. One store does vector + graph + full-text +
RRF in a single query (the reason it was chosen over pgvector for this project).

### Schema

```surql
-- Canonical sayings (truth)
DEFINE TABLE saying SCHEMAFULL;
DEFINE FIELD ref           ON saying TYPE string;         -- "Mark 12:17"
DEFINE FIELD book_author   ON saying TYPE string;
DEFINE FIELD text_original ON saying TYPE string;         -- WEB, public domain
DEFINE FIELD text_modern   ON saying TYPE string;         -- merged-LoRA target voice
DEFINE FIELD context       ON saying TYPE string;
DEFINE FIELD emb_original   ON saying TYPE array<float>;   -- Embedding Gemma
DEFINE FIELD emb_modern     ON saying TYPE array<float>;

DEFINE INDEX saying_vec_orig ON saying FIELDS emb_original
    HNSW DIMENSION 768 DIST COSINE;
DEFINE INDEX saying_vec_mod  ON saying FIELDS emb_modern
    HNSW DIMENSION 768 DIST COSINE;
DEFINE ANALYZER twin_an TOKENIZERS blank,class FILTERS lowercase,snowball(english);
DEFINE INDEX saying_ft ON saying FIELDS text_original, text_modern
    SEARCH ANALYZER twin_an BM25;

-- Graph nodes
DEFINE TABLE reasoning_move SCHEMAFULL;   -- M01..M18
DEFINE TABLE audience SCHEMAFULL;
DEFINE TABLE location SCHEMAFULL;
DEFINE TABLE concept SCHEMAFULL;

-- Graph edges (RELATE)
-- saying ->uses_move-> reasoning_move
-- saying ->spoken_to-> audience
-- saying ->at-> location
-- saying ->mentions-> concept
-- saying ->parallels-> saying          (synoptic parallels)
```

### Hybrid retrieval (one query)

```surql
LET $q = fn::embed($query);            -- via jesus-twin-inference embed()
-- vector candidates (match either register)
LET $vs = SELECT id, ref, text_modern,
    vector::similarity::cosine(emb_original, $q) AS s
  FROM saying WHERE emb_original <|20,COSINE|> $q ORDER BY s DESC LIMIT 20;
-- full-text candidates
LET $ft = SELECT id, ref, text_modern, search::score(0) AS s
  FROM saying WHERE text_modern @0@ $query ORDER BY s DESC LIMIT 20;
-- fuse, then expand by graph for richer context
LET $seeds = SELECT * FROM search::rrf([$vs, $ft], 5, 60);
RETURN SELECT *, ->uses_move->reasoning_move.* AS move,
                 ->parallels->saying.{ref, text_modern} AS parallels
       FROM $seeds;
```

- **Vector** finds semantically similar sayings; **graph** expansion
  (`uses_move`, `parallels`) surfaces structurally related ones pure similarity misses.
- `mindmap.rs` projects the same graph into nodes/edges for the mind-map UI and feeds
  `StateSnapshot` events.
- The coverage gate (`jesus-twin-core/gate.rs`) reads the fused top score; below threshold →
  `Refusal` before the model runs.

---

## 8. Skills (`jesus-twin-skills`)

```rust
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;                 // JSON schema (OpenAI tool format)
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json>;
}
```

One `Registry` is consumed by three frontends:
1. **CLI** (`jesus-twin skill <name> --args ...`) for local/offline use.
2. **MCP server** — each skill becomes an MCP tool (stdio + HTTP).
3. **Model tool-list** — skill schemas injected into the generation request so the
   twin can call them; results loop back as `ToolResult` events.

Built-in skills to start: `lookup_saying(ref)`, `find_by_move(M0x)`,
`parallels(ref)`, `mindmap(topic)`, `render_modern(ref)`. Each is a thin wrapper over
`jesus-twin-store` / `jesus-twin-inference`, so the same capability is reachable from CLI, MCP, and
the model itself.

---

## 9. Deployment modes (resolve the embedded-vs-scalable tension)

| Mode | Store | Engine | Use |
|---|---|---|---|
| **Edge / single binary** | SurrealDB embedded (in-process, `kv-surrealkv`) | mistral.rs in-process | Desktop app, offline, single user. Budget RAM: model weights + KV cache **and** SurrealDB compete for memory — acute on Mac unified memory. |
| **Scaled service** | SurrealDB remote node (same `Store` trait, `ws://`) | one or more inference workers behind the gatekeeper | Multi-user; store and inference scale independently. |

Because `jesus-twin-store::Store` is a trait, moving from embedded to remote is a config/impl
swap, not an agent-core change. Pick edge first to validate the full loop, then split the
store out when concurrency demands it.

---

## 10. Dependency & version notes

- **Pin the forks:** `mistral.rs` (GQAdonis), `prometheus-parking-lot-rs` (Prometheus-AGS),
  and any candle fork. Use exact git revs in the workspace `Cargo.toml`.
- **Candle coupling is the sharp edge.** mistral.rs pulls its own candle. If any other
  crate in the tree (or a future merge of candle-vllm code) also depends on candle, the
  revisions must match or you'll get duplicate-candle / trait-mismatch build errors. The
  candle-vllm fork already needed a `[patch."https://github.com/guoqingbao/candle.git"]`
  redirect — expect to manage similar patches here.
- **A2A / AG-UI Rust SDKs are young** (A2A SDKs more mature; AG-UI Rust is community
  tier). Vendor them behind your adapter modules so a spec bump is contained to one file.
- **SurrealDB 3.1** — confirm the embedded vector (HNSW) + BM25 + `search::rrf` features
  are enabled in the crate features you select.

---

## 11. Build sequence

1. **`jesus-twin-store` + data load** — port `build_training_jsonl.py` output into SurrealDB;
   build the vector indexes and the move/parallels graph; verify the hybrid retrieval
   query returns sane results.
2. **`jesus-twin-inference`** — embed mistral.rs as a library; serve Gemma 4 E4B (base, thinking
   off) and Embedding Gemma; map output to `AgentEvent`s.
3. **`jesus-twin-core`** — orchestrator (retrieve → gate → generate), coverage gate, the
   canonical event stream. Ship a RAG-first, base-model version first (no fine-tune).
4. **`jesus-twin-api` / OpenAI adapter** — get one surface working end-to-end with streaming.
5. **`jesus-twin-admission`** — wire the parking-lot gatekeeper in front of the engine.
6. **Remaining adapters** — MCP server (stdio + HTTP), then AG-UI, then A2A, each as a
   thin translation of the same event stream.
7. **`jesus-twin-skills` + `jesus-twin-cli`** — skill registry, expose via CLI + MCP + tool-list.
8. **Fine-tune & swap** — Unsloth LoRA on Gemma 4 E4B → merge → point `jesus-twin-inference` at
   the merged checkpoint. Keep it only if it improves style-by-move without hurting
   grounding (README §3, training_data_spec §5).

Ship value at step 3 (grounded RAG over one surface); everything after is surface area
and polish.
