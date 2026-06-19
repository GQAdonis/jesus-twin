//! `jesus-twin` CLI entry point.
//!
//! Subcommands (ARCHITECTURE.md §2, §8):
//!   - `serve`            — run the Axum app (all protocol surfaces).
//!   - `ingest <jsonl>`   — load the RAG corpus into an embedded SurrealDB store.
//!   - `retrieve <query>` — run a hybrid (BM25-for-now) retrieval and print the hits.
//!   - `ask <query>`      — run the full RAG orchestrator (retrieve → gate → generate).
//!   - `skill <name>`     — invoke a registered skill locally/offline (or list them).
//!   - `chat`             — interactive REPL against the orchestrator.
//!
//! `serve`/`ingest`/`retrieve`/`ask`/`skill` are live; `chat` is a stub. `serve` uses the real
//! Gemma 4 (mistral.rs) when built `--features mistralrs` (set `JESUS_TWIN_MODEL` to the merged
//! checkpoint — see RECIPE.md); otherwise the deterministic MockEngine. `ask`/`skill` use the
//! mock engine.

use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use jesus_twin_admission::{Gatekeeper, OpenGatekeeper, SemaphoreGatekeeper};
use jesus_twin_api::AppState;
use jesus_twin_core::event::AgentEvent;
use jesus_twin_core::event::Role;
use jesus_twin_core::gate::CoverageGate;
use jesus_twin_core::{Orchestrator, Session, Turn};
use jesus_twin_inference::MockEngine;
use jesus_twin_skills::{Registry, SkillCtx, register_builtins};
use jesus_twin_store::{Store, SurrealStore};
use uuid::Uuid;

/// Adapts the inference crate's `Embedder` to the store's `Embed` trait. The store deliberately
/// leaves this bridge to the binary (see its `embed.rs`) so the two leaf crates stay decoupled.
/// With it attached, ingest vectorizes the corpus and retrieval fuses BM25 + the embeddinggemma
/// vector leg via RRF instead of running BM25-only.
#[cfg(feature = "mistralrs")]
struct StoreEmbedder(Arc<jesus_twin_inference::MistralEngine>);

#[cfg(feature = "mistralrs")]
#[async_trait::async_trait]
impl jesus_twin_store::Embed for StoreEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, jesus_twin_store::StoreError> {
        use jesus_twin_inference::Embedder;
        self.0
            .embed(texts)
            .await
            .map_err(|e| jesus_twin_store::StoreError::Embedding(e.to_string()))
    }
}

/// Build the mistral.rs engine from env: `JESUS_TWIN_MODEL` (merged Gemma 4 checkpoint) and
/// `JESUS_TWIN_EMBED_MODEL` (embeddinggemma dir). Both fall back to the HF ids in
/// `MistralConfig::defaults()`; point them at local dirs for an offline release run. Loads
/// weights (multi-GB) — call once. Shared by serve/ask/ingest so all three get the same
/// 4-bit (ISQ Q4K) generation model and the same embedder.
#[cfg(feature = "mistralrs")]
async fn build_mistral_engine() -> anyhow::Result<Arc<jesus_twin_inference::MistralEngine>> {
    use jesus_twin_inference::{MistralConfig, MistralEngine};
    let defaults = MistralConfig::defaults();
    let model = std::env::var("JESUS_TWIN_MODEL").unwrap_or_else(|_| {
        tracing::warn!("JESUS_TWIN_MODEL unset; using the default base checkpoint");
        defaults.model.clone()
    });
    let embed_model = std::env::var("JESUS_TWIN_EMBED_MODEL").unwrap_or_else(|_| {
        tracing::warn!("JESUS_TWIN_EMBED_MODEL unset; using the default embeddinggemma id");
        defaults.embed_model.clone()
    });
    // In-situ quantization at load. `JESUS_TWIN_ISQ=none` serves full precision (diagnostic /
    // higher-VRAM hosts); anything else keeps the default 4-bit Q4K from `MistralConfig`.
    let isq = match std::env::var("JESUS_TWIN_ISQ").ok().as_deref() {
        Some("none") | Some("off") | Some("") => {
            tracing::warn!("JESUS_TWIN_ISQ=none — serving full precision (no ISQ)");
            None
        }
        _ => defaults.isq,
    };
    tracing::info!(%model, %embed_model, ?isq, "loading mistral.rs engine (downloads/loads weights)…");
    let engine = MistralEngine::build(MistralConfig {
        model,
        embed_model,
        isq,
    })
    .await?;
    Ok(Arc::new(engine))
}

#[derive(Parser)]
#[command(
    name = "jesus-twin",
    version,
    about = "Jesus digital twin agent service"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the agent service (OpenAI surface; MCP/AG-UI/A2A to follow).
    Serve {
        /// Address to bind, e.g. 127.0.0.1:8080.
        #[arg(long, env = "JESUS_TWIN_ADDR", default_value = "127.0.0.1:8080")]
        addr: String,
        /// Store directory; must already be ingested. Omit for an in-memory store ingested
        /// from `--jsonl` at startup.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Corpus to ingest into an in-memory store first (ignored when `--db` is set).
        #[arg(long, default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
    },
    /// Load the RAG corpus JSONL into the embedded store and build indexes.
    Ingest {
        /// Path to rag_corpus.jsonl.
        #[arg(default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
        /// Store directory (RocksDB). Omit for an ephemeral in-memory store.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
    },
    /// Retrieve sayings for a query (BM25 for now) and print the top hits.
    Retrieve {
        /// The query text.
        query: String,
        /// Store directory; must match a prior `ingest --db` (in-memory has no persistence).
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Max hits to return.
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Ask the twin: run the full RAG orchestrator (retrieve → gate → generate).
    Ask {
        /// The question.
        query: String,
        /// Store directory; must already be ingested. Omit to use an in-memory store, in
        /// which case `--jsonl` is ingested first.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Corpus to ingest into an in-memory store first (ignored when `--db` is set).
        #[arg(long, default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
    },
    /// Invoke a registered skill by name (or list skills with no name).
    Skill {
        /// Skill name, e.g. lookup_saying. Omit to list available skills.
        name: Option<String>,
        /// JSON arguments object, e.g. '{"ref":"Mark 12:17"}'.
        #[arg(long, default_value = "{}")]
        args: String,
        /// Store directory; omit for an in-memory store ingested from `--jsonl`.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Corpus for the in-memory store (ignored when `--db` is set).
        #[arg(long, default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
    },
    /// Interactive chat REPL.
    Chat,
    /// Gate diagnostics.
    Gate {
        #[command(subcommand)]
        cmd: GateCmd,
    },
    /// Generate doc2query-style MACHINE drafts of modern text into a sidecar (modern-legs-v1).
    /// Indexing-only — these revive the dead modern retrieval legs and are NEVER displayed or
    /// trained. Requires `--features mistralrs` (the generation model). Use `--limit` to sample.
    ModernDrafts {
        /// Corpus to draft modern renderings from.
        #[arg(default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
        /// Output sidecar path (`{id, ref, text_modern, machine_draft:true}` per line).
        #[arg(long, default_value = "../build/modern_drafts.jsonl")]
        out: String,
        /// Only draft the first N passages (0 = all). For sampling / verification.
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Apply machine-draft modern text from a sidecar into the store (flagging `machine_draft`)
    /// and re-embed so the modern retrieval legs go live. Retrieval-indexing only.
    ApplyModernDrafts {
        /// Sidecar produced by `modern-drafts`.
        #[arg(default_value = "../build/modern_drafts.jsonl")]
        jsonl: String,
        /// Store directory; must already be ingested.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
    },
    /// Ingest the Tanakh (JPS 1917, his source material) into the separate `tanakh` table and
    /// embed it. Produce the JSONL first with `python ingest_tanakh.py --out build/tanakh.jsonl`.
    IngestTanakh {
        /// Tanakh JSONL produced by `ingest_tanakh.py`.
        #[arg(default_value = "../build/tanakh.jsonl")]
        jsonl: String,
        /// Store directory; must already be ingested with the red-letter corpus.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
    },
    /// Retrieve Tanakh source verses for a query — labeled as HIS SOURCE MATERIAL, not his words.
    RetrieveTanakh {
        /// The query text.
        query: String,
        /// Store directory; must have an ingested `tanakh` table.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Max verses to return.
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Machine-tag every saying with life-domain + principle facets into a sidecar
    /// (principle-index-v1). Retrieval-metadata only — never displayed or trained. Requires
    /// `--features mistralrs`. Use `--limit` to sample.
    PrincipleTag {
        /// Corpus to tag.
        #[arg(default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
        /// Output sidecar (`{id, ref, domains, principles, machine_tagged:true}` per line).
        #[arg(long, default_value = "../build/principle_tags.jsonl")]
        out: String,
        /// Only tag the first N passages (0 = all). For sampling / verification.
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Apply principle-index facets from a sidecar into the store (flagging `machine_tagged`).
    ApplyPrincipleTags {
        /// Sidecar produced by `principle-tag`.
        #[arg(default_value = "../build/principle_tags.jsonl")]
        jsonl: String,
        /// Store directory; must already be ingested.
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
    },
}

#[derive(Subcommand)]
enum GateCmd {
    /// Run every eval query through the retrieval legs and write a leg-agreement report
    /// (`docs/gate-calibration-claude-code-prompt.md`, Assess phase). Read-only: it does not
    /// change retrieval or the gate. Run with `--features mistralrs` + an ingested `--db` on
    /// the GPU box for the real 4-leg numbers; without the feature it reports the BM25-only
    /// path (1–2 legs) and says so.
    Calibrate {
        /// Directory holding the eval JSONL files (grounding, refusal, method-application,
        /// boundary).
        #[arg(long, default_value = "../eval")]
        eval_dir: String,
        /// Store directory; must already be ingested (with embeddings for the 4-leg path).
        #[arg(long, env = "JESUS_TWIN_DB")]
        db: Option<String>,
        /// Corpus for an in-memory store when `--db` is omitted.
        #[arg(long, default_value = "../build/rag_corpus.jsonl")]
        jsonl: String,
        /// Where to write the calibration report JSONL.
        #[arg(long, default_value = "../eval/out/gate-calibration.jsonl")]
        out: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve { addr, db, jsonl } => serve(&addr, db.as_deref(), &jsonl).await,
        Command::Ingest { jsonl, db } => ingest(&jsonl, db.as_deref()).await,
        Command::Retrieve { query, db, limit } => retrieve(&query, db.as_deref(), limit).await,
        Command::Ask { query, db, jsonl } => ask(&query, db.as_deref(), &jsonl).await,
        Command::Skill {
            name,
            args,
            db,
            jsonl,
        } => skill(name.as_deref(), &args, db.as_deref(), &jsonl).await,
        Command::Gate {
            cmd:
                GateCmd::Calibrate {
                    eval_dir,
                    db,
                    jsonl,
                    out,
                },
        } => gate_calibrate(&eval_dir, db.as_deref(), &jsonl, &out).await,
        Command::ModernDrafts { jsonl, out, limit } => modern_drafts(&jsonl, &out, limit).await,
        Command::ApplyModernDrafts { jsonl, db } => {
            apply_modern_drafts(&jsonl, db.as_deref()).await
        }
        Command::IngestTanakh { jsonl, db } => ingest_tanakh(&jsonl, db.as_deref()).await,
        Command::RetrieveTanakh { query, db, limit } => {
            retrieve_tanakh(&query, db.as_deref(), limit).await
        }
        Command::PrincipleTag { jsonl, out, limit } => principle_tag(&jsonl, &out, limit).await,
        Command::ApplyPrincipleTags { jsonl, db } => {
            apply_principle_tags(&jsonl, db.as_deref()).await
        }
        Command::Chat => {
            // TODO(build step 3+): drive the orchestrator interactively.
            anyhow::bail!("chat REPL not yet implemented");
        }
    }
}

async fn serve(addr: &str, db: Option<&str>, jsonl: &str) -> anyhow::Result<()> {
    // The engine is the only thing the `mistralrs` feature changes: the real Gemma 4 served by
    // mistral.rs vs the deterministic mock. Both implement Engine, so everything downstream —
    // orchestrator, gate, adapters, admission, skills — is identical. The real build also
    // attaches the engine as the store's embedder (embeddinggemma), upgrading retrieval from
    // BM25-only to hybrid BM25 + vector + RRF.
    #[cfg(feature = "mistralrs")]
    {
        // Engine first so it can also drive the store's vector leg; ingest *after* attaching it
        // so an in-memory corpus is vectorized (a persistent `--db` must be ingested likewise).
        let engine = build_mistral_engine().await?;
        let store: Arc<SurrealStore> = Arc::new(
            open_store(db)
                .await?
                .with_embedder(Arc::new(StoreEmbedder(engine.clone()))),
        );
        if db.is_none() {
            store.ingest_corpus(jsonl).await?;
        }
        serve_with(addr, store, engine).await
    }
    #[cfg(not(feature = "mistralrs"))]
    {
        // Share one store handle (Arc) between the orchestrator and the skill context so MCP can
        // surface the skills without a second ingest. No real embedder → BM25-only retrieval.
        let store: Arc<SurrealStore> = Arc::new(open_store(db).await?);
        if db.is_none() {
            store.ingest_corpus(jsonl).await?;
        }
        serve_with(addr, store, Arc::new(MockEngine::new())).await
    }
}

/// Serve over `store` + `engine`. Generic over the engine so the `mistralrs` feature can swap
/// the real Gemma in without touching the rest of the wiring.
async fn serve_with<E>(addr: &str, store: Arc<SurrealStore>, engine: Arc<E>) -> anyhow::Result<()>
where
    E: jesus_twin_inference::Engine + 'static,
{
    // Bounded admission control for the concurrent served path (ARCHITECTURE.md §6).
    let gatekeeper = SemaphoreGatekeeper::new(8, 64, Duration::from_secs(30));
    let registry = register_builtins(Registry::new());
    let orch = Orchestrator::new(
        store.clone(),
        engine.clone(),
        gatekeeper,
        registry.clone(),
        CoverageGate,
    );
    let skill_ctx = Arc::new(SkillCtx::new(store, engine));
    let state = AppState::new(Arc::new(orch)).with_skills(registry, skill_ctx);

    tracing::info!(%addr, "jesus-twin listening");
    jesus_twin_api::serve(addr, state).await?;
    Ok(())
}

/// Build the orchestrator over `gatekeeper` with the mock engine (RAG-first build, no weights).
/// Retained as the canonical mock-orchestrator constructor for future subcommands; not yet
/// called (pre-existing — predates the gate-calibration work).
#[allow(dead_code)]
async fn build_orchestrator_mock<G: Gatekeeper + 'static>(
    db: Option<&str>,
    jsonl: &str,
    gatekeeper: G,
) -> anyhow::Result<Orchestrator<SurrealStore, MockEngine, G>> {
    let store = open_store(db).await?;
    if db.is_none() {
        store.ingest_corpus(jsonl).await?;
    }
    Ok(Orchestrator::new(
        store,
        MockEngine::new(),
        gatekeeper,
        register_builtins(Registry::new()),
        CoverageGate,
    ))
}

/// List the built-in skills, or invoke one by name with JSON `args`. Builds a read-only
/// registry over the store + mock engine (the same RAG-first wiring as `ask`).
async fn skill(
    name: Option<&str>,
    args: &str,
    db: Option<&str>,
    jsonl: &str,
) -> anyhow::Result<()> {
    let registry = register_builtins(Registry::new());

    let Some(name) = name else {
        println!("available skills:");
        for n in registry.names() {
            println!("  {n}");
        }
        return Ok(());
    };

    let store = open_store(db).await?;
    if db.is_none() {
        store.ingest_corpus(jsonl).await?;
    }
    let ctx = SkillCtx::new(Arc::new(store), Arc::new(MockEngine::new()));
    let parsed: serde_json::Value =
        serde_json::from_str(args).map_err(|e| anyhow::anyhow!("invalid --args JSON: {e}"))?;

    let result = registry.invoke(name, parsed, &ctx).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// Open a store (persistent if `db` is set, else in-memory) and ingest the corpus.
async fn ingest(jsonl: &str, db: Option<&str>) -> anyhow::Result<()> {
    // With the `mistralrs` feature, attach the embeddinggemma embedder so ingest also writes the
    // HNSW vectors (`emb_original`/`emb_modern`) that hybrid retrieval needs — re-ingesting a
    // BM25-only `--db` upgrades it in place (embed_all is idempotent). Loading the engine here
    // also loads the generation model; that's wasted work for a pure ingest, but keeps one
    // engine constructor. Without the feature, ingest stays BM25-only.
    #[cfg(feature = "mistralrs")]
    let store = {
        let engine = build_mistral_engine().await?;
        open_store(db)
            .await?
            .with_embedder(Arc::new(StoreEmbedder(engine)))
    };
    #[cfg(not(feature = "mistralrs"))]
    let store = open_store(db).await?;

    let count = store.ingest_corpus(jsonl).await?;
    // Drop the store and yield so the embedded engine settles before exit (surrealdb#2399);
    // avoids a cosmetic "transaction dropped" log. Data is already committed per-statement.
    drop(store);
    tokio::task::yield_now().await;
    println!("ingested {count} passages from {jsonl}");
    if db.is_none() {
        println!("(in-memory store — nothing persisted; pass --db to keep it)");
    }
    Ok(())
}

/// Gate-calibration instrument (Assess phase of `docs/gate-calibration-claude-code-prompt.md`).
///
/// Runs every query in the eval sets through the retrieval legs and records, per query, the
/// fused top score, how many legs ranked the top passage (`top_legs_matched`), the live-leg
/// count, and the top-3 ids. Writes a JSONL report and prints per-set leg-agreement
/// distributions. Read-only: it changes neither retrieval nor the gate. With `--features
/// mistralrs` + an ingested `--db` it reports the real 4-leg numbers; otherwise the BM25-only
/// path, which it labels so the distributions aren't misread.
async fn gate_calibrate(
    eval_dir: &str,
    db: Option<&str>,
    jsonl: &str,
    out: &str,
) -> anyhow::Result<()> {
    use std::io::Write;

    #[cfg(feature = "mistralrs")]
    let store = {
        let engine = build_mistral_engine().await?;
        let s = open_store(db)
            .await?
            .with_embedder(Arc::new(StoreEmbedder(engine)));
        if db.is_none() {
            s.ingest_corpus(jsonl).await?;
        }
        s
    };
    #[cfg(not(feature = "mistralrs"))]
    let store = {
        let s = open_store(db).await?;
        if db.is_none() {
            s.ingest_corpus(jsonl).await?;
        }
        eprintln!(
            "WARNING: built without --features mistralrs → BM25-only retrieval (1–2 legs). \
             The 4-leg calibration the gate design needs requires the embeddinggemma embedder \
             on a GPU host. Run there for production numbers."
        );
        s
    };

    // The four eval sets the doc names, with the query key each uses.
    let sets = [
        ("grounding", "grounding.jsonl"),
        ("refusal", "refusal.jsonl"),
        ("method-application", "method-application.jsonl"),
        ("boundary", "boundary.jsonl"),
    ];

    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut report = std::fs::File::create(out)?;

    for (set_name, file) in sets {
        let path = format!("{eval_dir}/{file}");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skip {set_name}: cannot read {path}: {e}");
                continue;
            }
        };
        let mut dist: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
        let mut n = 0usize;
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line)?;
            // Eval sets use either `user_query` or `query`.
            let query = v
                .get("user_query")
                .or_else(|| v.get("query"))
                .and_then(|q| q.as_str())
                .unwrap_or("");
            if query.is_empty() {
                continue;
            }
            let row = store.calibrate_query(query, 5).await?;
            *dist.entry(row.top_legs_matched).or_insert(0) += 1;
            n += 1;
            let mut line = serde_json::to_value(&row)?;
            line["set"] = serde_json::Value::String(set_name.to_string());
            line["eval_id"] = v.get("id").cloned().unwrap_or(serde_json::Value::Null);
            writeln!(report, "{}", serde_json::to_string(&line)?)?;
        }
        let summary: Vec<String> = dist
            .iter()
            .map(|(legs, count)| format!("{legs}leg={count}"))
            .collect();
        println!(
            "{set_name:<20} n={n:<3} legs_matched: [{}]",
            summary.join(" ")
        );
    }

    drop(store);
    tokio::task::yield_now().await;
    println!("\ncalibration report written to {out}");
    println!("Next (Plan phase): apply the preregistered tier rule to these distributions.");
    Ok(())
}

/// Retrieve and print the top hits with their citation refs and scores.
async fn retrieve(query: &str, db: Option<&str>, limit: usize) -> anyhow::Result<()> {
    let store = open_store(db).await?;
    let set = store.retrieve(query, limit).await?;
    if set.passages.is_empty() {
        println!("no coverage for \"{query}\" — the recorded teachings don't address that.");
        return Ok(());
    }
    for p in &set.passages {
        let score = p.score.unwrap_or(0.0);
        println!("[{score:.3}] {}  {}", p.ref_, p.text_original);
    }
    Ok(())
}

/// Run the full RAG orchestrator for one question and print the resulting event stream.
/// Uses the real Gemma 4 engine when built `--features mistralrs` (set `JESUS_TWIN_MODEL`);
/// falls back to the deterministic mock otherwise.
async fn ask(query: &str, db: Option<&str>, jsonl: &str) -> anyhow::Result<()> {
    let session = Session::new(Uuid::new_v4()).with_turn(Turn::new(Role::User, query));

    #[cfg(feature = "mistralrs")]
    {
        // Engine first so it doubles as the store's embedder (hybrid retrieval), then ingest.
        let engine = build_mistral_engine().await?;
        let store = open_store(db)
            .await?
            .with_embedder(Arc::new(StoreEmbedder(engine.clone())));
        if db.is_none() {
            store.ingest_corpus(jsonl).await?;
        }
        let orch = Orchestrator::new(
            store,
            engine,
            OpenGatekeeper,
            register_builtins(Registry::new()),
            CoverageGate,
        );
        for event in orch.run(&session).await? {
            print_event(&event);
        }
    }

    #[cfg(not(feature = "mistralrs"))]
    {
        let store = open_store(db).await?;
        if db.is_none() {
            store.ingest_corpus(jsonl).await?;
        }
        let orch = Orchestrator::new(
            store,
            MockEngine::new(),
            OpenGatekeeper,
            register_builtins(Registry::new()),
            CoverageGate,
        );
        for event in orch.run(&session).await? {
            print_event(&event);
        }
    }

    Ok(())
}

/// Render one `AgentEvent` for the terminal (the OpenAI/AG-UI adapters do this for the wire).
fn print_event(event: &AgentEvent) {
    match event {
        AgentEvent::Citation { ref_, score, .. } => println!("  · cite {ref_} ({score:.3})"),
        AgentEvent::TextMessageDelta { delta, .. } => println!("\n{delta}\n"),
        AgentEvent::Refusal { reason } => {
            println!("\n[refused: {reason:?}] the recorded teachings don't address that.\n")
        }
        AgentEvent::Custom { name, data } => {
            // Surface the Tier-2 low-confidence flag (and any future namespaced chunk).
            println!("  ⚠ {name} {data}");
        }
        AgentEvent::RunFinished { finish, .. } => println!("  ({finish:?})"),
        _ => {}
    }
}

/// Generate machine-draft modern renderings (modern-legs-v1) into a sidecar JSONL. Plain
/// generation (NOT the twin orchestrator): each saying's `text_original` is rewritten in modern
/// English to populate the modern-register retrieval legs. The output is flagged
/// `machine_draft: true` and is consumed only by `apply-modern-drafts` for indexing — never
/// displayed (`context_lines` uses `text_original`) or trained (SFT reads the human xlsx).
#[cfg(feature = "mistralrs")]
async fn modern_drafts(jsonl: &str, out: &str, limit: usize) -> anyhow::Result<()> {
    use jesus_twin_inference::{Engine, GenRequest};
    use std::io::Write;

    const DRAFT_SYSTEM: &str = "Rewrite the given saying in plain, natural, present-day English. \
Preserve the exact meaning and any concrete imagery. Do NOT add commentary, framing, names, \
explanation, or new content. Output only the rewritten line.";

    let engine = build_mistral_engine().await?;
    let content = std::fs::read_to_string(jsonl)?;
    let mut records: Vec<jesus_twin_store::RagRecord> = Vec::new();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        records.push(serde_json::from_str(line).map_err(|e| anyhow::anyhow!("{jsonl}: {e}"))?);
    }
    if limit > 0 {
        records.truncate(limit);
    }
    let total = records.len();
    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut f = std::fs::File::create(out)?;
    for (i, rec) in records.iter().enumerate() {
        let draft = engine
            .generate(GenRequest {
                system: DRAFT_SYSTEM.to_string(),
                context: String::new(),
                user: rec.text_original.clone(),
            })
            .await?;
        let row = serde_json::json!({
            "id": rec.id,
            "ref": rec.ref_,
            "text_modern": draft.trim(),
            "machine_draft": true,
        });
        writeln!(f, "{}", serde_json::to_string(&row)?)?;
        if (i + 1) % 25 == 0 || i + 1 == total {
            tracing::info!("drafted {}/{}", i + 1, total);
        }
    }
    println!("wrote {total} machine-draft modern renderings to {out}");
    println!("next: apply-modern-drafts {out} --db <store>");
    Ok(())
}

#[cfg(not(feature = "mistralrs"))]
async fn modern_drafts(_jsonl: &str, _out: &str, _limit: usize) -> anyhow::Result<()> {
    anyhow::bail!(
        "modern-drafts requires building --features mistralrs (needs the generation model)"
    )
}

/// Apply a machine-draft sidecar into the store and re-embed the modern legs. Builds the engine
/// (under the feature) so the embedder is attached and `emb_modern` is populated.
async fn apply_modern_drafts(jsonl: &str, db: Option<&str>) -> anyhow::Result<()> {
    #[cfg(feature = "mistralrs")]
    let store = {
        let engine = build_mistral_engine().await?;
        open_store(db)
            .await?
            .with_embedder(Arc::new(StoreEmbedder(engine)))
    };
    #[cfg(not(feature = "mistralrs"))]
    let store = open_store(db).await?;

    let count = store.ingest_modern_drafts(jsonl).await?;
    drop(store);
    tokio::task::yield_now().await;
    println!("applied {count} machine-draft modern renderings (modern retrieval legs re-embedded)");
    Ok(())
}

/// Ingest the Tanakh JSONL (his source material) into the `tanakh` table + embed it. Builds the
/// engine (under the feature) so the embedder populates `emb`.
async fn ingest_tanakh(jsonl: &str, db: Option<&str>) -> anyhow::Result<()> {
    #[cfg(feature = "mistralrs")]
    let store = {
        let engine = build_mistral_engine().await?;
        open_store(db)
            .await?
            .with_embedder(Arc::new(StoreEmbedder(engine)))
    };
    #[cfg(not(feature = "mistralrs"))]
    let store = open_store(db).await?;

    let count = store.ingest_tanakh(jsonl).await?;
    drop(store);
    tokio::task::yield_now().await;
    println!("ingested {count} Tanakh verses (his source material — not his words)");
    Ok(())
}

/// Retrieve Tanakh source verses, labeled distinctly. BM25-only here (no embedder attached);
/// `serve`/`ask` get the vector leg too. Prints with a clear "source material" header.
async fn retrieve_tanakh(query: &str, db: Option<&str>, limit: usize) -> anyhow::Result<()> {
    let store = open_store(db).await?;
    let hits = store.retrieve_tanakh(query, limit).await?;
    if hits.is_empty() {
        println!("no Tanakh source material matches \"{query}\".");
        return Ok(());
    }
    println!("Source material — Hebrew Bible (JPS 1917), what he drew on; NOT his own words:");
    for p in &hits {
        let score = p.score.unwrap_or(0.0);
        println!("  [{score:.3}] {} ({}) {}", p.ref_, p.category, p.text);
    }
    Ok(())
}

/// The ~20 life-domain taxonomy (principle-index-v1; plan Change 11). Machine tagging picks from
/// this fixed set so a question about a domain can boost passages tagged with it. Used by the
/// `mistralrs`-gated tagger and the parser tests.
#[cfg(any(feature = "mistralrs", test))]
const TAXONOMY: &[&str] = &[
    "money/provision",
    "fear/anxiety",
    "grief",
    "marriage/divorce",
    "parenting",
    "conflict/forgiveness",
    "ambition/status",
    "honesty",
    "illness",
    "purpose/calling",
    "enemies",
    "doubt",
    "prayer",
    "wealth/generosity",
    "judgment of others",
    "work",
    "power",
    "loneliness",
    "temptation",
    "death",
];

/// Parse a tagging reply into canonical domains (matched against [`TAXONOMY`]) + principle lines.
/// Lenient: tolerates casing/extra prose; keeps only domains in the taxonomy. Pure — unit-tested.
#[cfg(any(feature = "mistralrs", test))]
fn parse_principle_tags(reply: &str) -> (Vec<String>, Vec<String>) {
    let mut domains = Vec::new();
    let mut principles = Vec::new();
    for line in reply.lines() {
        let l = line.trim();
        let low = l.to_lowercase();
        if let Some(rest) = low.strip_prefix("domains:") {
            for part in rest.split(',') {
                let p = part.trim();
                if let Some(canon) = TAXONOMY
                    .iter()
                    .find(|t| **t == p || p.contains(*t) || t.contains(p))
                {
                    if !domains.iter().any(|d| d == canon) {
                        domains.push((*canon).to_string());
                    }
                }
            }
        } else if let Some(_rest) = low.strip_prefix("principle:") {
            // Preserve original casing of the principle text.
            let p = l[l.to_lowercase().find("principle:").unwrap() + "principle:".len()..].trim();
            if !p.is_empty() {
                principles.push(p.to_string());
            }
        }
    }
    (domains, principles)
}

/// Machine-tag sayings with life-domain + principle facets (principle-index-v1) into a sidecar.
/// Plain generation; retrieval-metadata only — never displayed or trained.
#[cfg(feature = "mistralrs")]
async fn principle_tag(jsonl: &str, out: &str, limit: usize) -> anyhow::Result<()> {
    use jesus_twin_inference::{Engine, GenRequest};
    use std::io::Write;

    let system = format!(
        "You label a saying with the life DOMAINS it speaks to and the governing PRINCIPLE it \
establishes. Choose domains ONLY from this list: {}. State one short principle, derived from the \
saying itself — never invent beyond it. Output EXACTLY two lines and nothing else:\n\
DOMAINS: <comma-separated domains from the list>\n\
PRINCIPLE: <one short sentence>",
        TAXONOMY.join(", ")
    );

    let engine = build_mistral_engine().await?;
    let content = std::fs::read_to_string(jsonl)?;
    let mut records: Vec<jesus_twin_store::RagRecord> = Vec::new();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        records.push(serde_json::from_str(line).map_err(|e| anyhow::anyhow!("{jsonl}: {e}"))?);
    }
    if limit > 0 {
        records.truncate(limit);
    }
    let total = records.len();
    if let Some(parent) = std::path::Path::new(out).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let mut f = std::fs::File::create(out)?;
    for (i, rec) in records.iter().enumerate() {
        let reply = engine
            .generate(GenRequest {
                system: system.clone(),
                context: String::new(),
                user: rec.text_original.clone(),
            })
            .await?;
        let (domains, principles) = parse_principle_tags(&reply);
        let row = serde_json::json!({
            "id": rec.id,
            "ref": rec.ref_,
            "domains": domains,
            "principles": principles,
            "machine_tagged": true,
        });
        writeln!(f, "{}", serde_json::to_string(&row)?)?;
        if (i + 1) % 25 == 0 || i + 1 == total {
            tracing::info!("tagged {}/{}", i + 1, total);
        }
    }
    println!("wrote {total} principle-index tags to {out}");
    println!("next: apply-principle-tags {out} --db <store>");
    Ok(())
}

#[cfg(not(feature = "mistralrs"))]
async fn principle_tag(_jsonl: &str, _out: &str, _limit: usize) -> anyhow::Result<()> {
    anyhow::bail!(
        "principle-tag requires building --features mistralrs (needs the generation model)"
    )
}

/// Apply a principle-index sidecar into the store (no embedder needed — facets don't embed).
async fn apply_principle_tags(jsonl: &str, db: Option<&str>) -> anyhow::Result<()> {
    let store = open_store(db).await?;
    let count = store.ingest_principle_tags(jsonl).await?;
    drop(store);
    tokio::task::yield_now().await;
    println!("tagged {count} sayings with life-domain + principle facets");
    Ok(())
}

async fn open_store(db: Option<&str>) -> anyhow::Result<SurrealStore> {
    Ok(match db {
        Some(path) => SurrealStore::open(path).await?,
        None => SurrealStore::memory().await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_principle_tags_extracts_canonical_domains_and_principle() {
        let reply = "DOMAINS: fear/anxiety, money/provision, made-up-domain\n\
                     PRINCIPLE: God provides; worry changes nothing.";
        let (domains, principles) = parse_principle_tags(reply);
        assert_eq!(domains, vec!["fear/anxiety", "money/provision"]); // bogus domain dropped
        assert_eq!(principles, vec!["God provides; worry changes nothing."]);
    }

    #[test]
    fn parse_principle_tags_is_lenient_about_casing_and_noise() {
        let reply =
            "Here are the tags.\ndomains: Prayer\nprinciple: Ask, and keep asking.\nthanks!";
        let (domains, principles) = parse_principle_tags(reply);
        assert_eq!(domains, vec!["prayer"]);
        assert_eq!(principles, vec!["Ask, and keep asking."]);
    }
}
