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
        let store: Arc<SurrealStore> =
            Arc::new(open_store(db).await?.with_embedder(Arc::new(StoreEmbedder(engine.clone()))));
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
        CoverageGate::default(),
    );
    let skill_ctx = Arc::new(SkillCtx::new(store, engine));
    let state = AppState::new(Arc::new(orch)).with_skills(registry, skill_ctx);

    tracing::info!(%addr, "jesus-twin listening");
    jesus_twin_api::serve(addr, state).await?;
    Ok(())
}

/// Build the orchestrator over `gatekeeper` with the mock engine (RAG-first build, no weights).
/// Used by `skill` and `retrieve` where the real engine isn't needed.
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
        CoverageGate::default(),
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
            CoverageGate::default(),
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
            CoverageGate::default(),
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
        AgentEvent::RunFinished { finish, .. } => println!("  ({finish:?})"),
        _ => {}
    }
}

async fn open_store(db: Option<&str>) -> anyhow::Result<SurrealStore> {
    Ok(match db {
        Some(path) => SurrealStore::open(path).await?,
        None => SurrealStore::memory().await?,
    })
}
