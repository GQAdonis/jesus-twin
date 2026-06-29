//! Integration tests for the skill registry: authorized invoke, authz denial, listing.
//!
//! Uses lightweight doubles for `Store`/`Engine` so the skill plumbing + the authorization
//! boundary are tested without a real backend.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use jesus_twin_inference::{Engine, EngineError, GenRequest};
use jesus_twin_skills::{
    Authorizer, Decision, Registry, RiskClass, Skill, SkillCtx, SkillError, register_builtins,
};
use jesus_twin_store::{Passage, RetrievalSet, Store, StoreError};

/// A store double returning a canned Caesar saying.
struct FakeStore;

#[async_trait]
impl Store for FakeStore {
    async fn retrieve(&self, _q: &str, _limit: usize) -> Result<RetrievalSet, StoreError> {
        Ok(RetrievalSet {
            passages: vec![caesar()],
            ..Default::default()
        })
    }
    async fn ingest_corpus(&self, _p: &str) -> Result<usize, StoreError> {
        Ok(0)
    }
    async fn get_by_ref(&self, r: &str) -> Result<Option<Passage>, StoreError> {
        Ok((r == "Mark 12:17").then(caesar))
    }
    async fn find_by_move(&self, m: &str, _limit: usize) -> Result<Vec<Passage>, StoreError> {
        Ok(if m == "M02" { vec![caesar()] } else { vec![] })
    }
}

fn caesar() -> Passage {
    Passage {
        id: "wj-1".into(),
        ref_: "Mark 12:17".into(),
        book_author: "Mark".into(),
        text_original: "Give to Caesar what is Caesar's.".into(),
        text_modern: String::new(),
        context: String::new(),
        location: String::new(),
        occasion: String::new(),
        move_: "M02".into(),
        translation: String::new(),
        domains: Vec::new(),
        principles: Vec::new(),
        score: Some(8.8),
    }
}

struct FakeEngine;

#[async_trait]
impl Engine for FakeEngine {
    async fn generate(&self, req: GenRequest) -> Result<String, EngineError> {
        Ok(format!("(rendered) {}", req.context))
    }
}

fn ctx() -> SkillCtx {
    SkillCtx::new(Arc::new(FakeStore), Arc::new(FakeEngine))
}

#[tokio::test]
async fn lookup_saying_returns_the_cited_line() {
    let reg = register_builtins(Registry::new());
    let out = reg
        .invoke("lookup_saying", json!({ "ref": "Mark 12:17" }), &ctx())
        .await
        .unwrap();
    assert_eq!(out["ref"], "Mark 12:17");
    assert!(out["text_original"].as_str().unwrap().contains("Caesar"));
}

#[tokio::test]
async fn render_modern_is_grounded_on_the_original() {
    let reg = register_builtins(Registry::new());
    let out = reg
        .invoke("render_modern", json!({ "ref": "Mark 12:17" }), &ctx())
        .await
        .unwrap();
    // The mock engine echoes the context, which is the cited original — proving grounding.
    assert!(out["modern"].as_str().unwrap().contains("Caesar"));
    assert_eq!(out["ref"], "Mark 12:17");
}

#[tokio::test]
async fn find_by_move_lists_tagged_sayings() {
    let reg = register_builtins(Registry::new());
    let out = reg
        .invoke("find_by_move", json!({ "move": "M02" }), &ctx())
        .await
        .unwrap();
    assert_eq!(out["count"], 1);
}

#[tokio::test]
async fn mindmap_projects_nodes_and_edges() {
    let reg = register_builtins(Registry::new());
    let out = reg
        .invoke("mindmap", json!({ "topic": "Caesar" }), &ctx())
        .await
        .unwrap();
    // FakeStore::retrieve returns one (annotated) saying -> topic + saying + move nodes.
    let nodes = out["nodes"].as_array().unwrap();
    assert!(nodes.iter().any(|n| n["kind"] == "topic"));
    assert!(
        nodes
            .iter()
            .any(|n| n["kind"] == "saying" && n["label"] == "Mark 12:17")
    );
    assert!(
        out["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["relation"] == "matches")
    );
}

#[tokio::test]
async fn unknown_skill_errors() {
    let reg = register_builtins(Registry::new());
    let err = reg.invoke("nope", json!({}), &ctx()).await.unwrap_err();
    assert!(matches!(err, SkillError::Unknown(_)));
}

#[tokio::test]
async fn missing_arg_is_invalid() {
    let reg = register_builtins(Registry::new());
    let err = reg
        .invoke("lookup_saying", json!({}), &ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, SkillError::InvalidArgs(_)));
}

/// A risky (outbound) skill used only to prove the authorization boundary blocks it.
struct DangerSkill;

#[async_trait]
impl Skill for DangerSkill {
    fn name(&self) -> &str {
        "send_message"
    }
    fn description(&self) -> &str {
        "Sends a message externally (outbound)."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::Outbound
    }
    fn schema(&self) -> Value {
        json!({ "type": "object" })
    }
    async fn invoke(&self, _args: Value, _ctx: &SkillCtx) -> Result<Value, SkillError> {
        Ok(json!({ "sent": true })) // must NEVER be reached under the default policy
    }
}

#[tokio::test]
async fn outbound_skill_is_denied_by_default_policy() {
    let reg = register_builtins(Registry::new()).with(Arc::new(DangerSkill));
    let err = reg
        .invoke("send_message", json!({}), &ctx())
        .await
        .unwrap_err();
    assert!(
        matches!(err, SkillError::NotAuthorized(_)),
        "persona != permission: outbound skills must be denied without an approval channel"
    );
}

#[tokio::test]
async fn human_checkpoint_can_allow_outbound() {
    // An explicit approval channel that approves: the skill now runs.
    struct ApproveAll;
    impl Authorizer for ApproveAll {
        fn authorize(&self, _skill: &str, _risk: RiskClass) -> Decision {
            Decision::Allow
        }
    }
    let reg = Registry::new()
        .with_authorizer(Arc::new(ApproveAll))
        .with(Arc::new(DangerSkill));
    let out = reg.invoke("send_message", json!({}), &ctx()).await.unwrap();
    assert_eq!(out["sent"], true);
}

#[tokio::test]
async fn lists_builtin_skills() {
    let reg = register_builtins(Registry::new());
    let names = reg.names();
    for expected in [
        "lookup_saying",
        "find_by_move",
        "parallels",
        "render_modern",
    ] {
        assert!(names.contains(&expected), "missing skill: {expected}");
    }
}
