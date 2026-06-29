//! Graph projections for the mind-map view.
//!
//! Projects the graphrag store into nodes/edges that feed the `MINDMAP_DELTA` /
//! `STATE_SNAPSHOT` events (ARCHITECTURE.md §7, ALIGNMENT_AND_TUNING.md §4).

use serde::{Deserialize, Serialize};

use crate::retrieve::Passage;

/// A node in the mind-map (a saying, move, audience, location, or concept).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindmapNode {
    pub id: String,
    pub label: String,
    pub kind: String,
}

/// A directed edge between two nodes (`uses_move`, `parallels`, `mentions`, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindmapEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

/// A graph fragment streamed to the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MindmapDelta {
    pub nodes: Vec<MindmapNode>,
    pub edges: Vec<MindmapEdge>,
}

/// Build a mind-map around `topic` from its retrieved `passages`:
/// - a central `topic` node, with `matches` edges to each saying;
/// - a `saying` node per passage (labelled by its `ref`);
/// - a `reasoning_move` node per distinct move, with `uses_move` edges from its sayings;
/// - `parallels` edges between sayings that share a move (structural relatedness).
///
/// Pure function over the retrieval set — unit-tested, no DB. Moves are only present for
/// annotated sayings, so the move/parallels layer fills in as annotation lands.
pub fn project_topic(topic: &str, passages: &[Passage]) -> MindmapDelta {
    let mut delta = MindmapDelta::default();
    let topic_id = format!("topic:{topic}");
    delta.nodes.push(MindmapNode {
        id: topic_id.clone(),
        label: topic.to_string(),
        kind: "topic".to_string(),
    });

    // Saying nodes + topic->saying edges. Track sayings per move for the parallels layer.
    let mut by_move: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for p in passages {
        let sid = format!("saying:{}", p.id);
        delta.nodes.push(MindmapNode {
            id: sid.clone(),
            label: p.ref_.clone(),
            kind: "saying".to_string(),
        });
        delta.edges.push(MindmapEdge {
            from: topic_id.clone(),
            to: sid.clone(),
            relation: "matches".to_string(),
        });
        if !p.move_.trim().is_empty() {
            by_move.entry(p.move_.clone()).or_default().push(sid);
        }
    }

    // Move nodes + uses_move edges, and parallels edges between co-move sayings.
    for (mv, sayings) in &by_move {
        let mid = format!("move:{mv}");
        delta.nodes.push(MindmapNode {
            id: mid.clone(),
            label: mv.clone(),
            kind: "reasoning_move".to_string(),
        });
        for sid in sayings {
            delta.edges.push(MindmapEdge {
                from: sid.clone(),
                to: mid.clone(),
                relation: "uses_move".to_string(),
            });
        }
        for pair in sayings.windows(2) {
            delta.edges.push(MindmapEdge {
                from: pair[0].clone(),
                to: pair[1].clone(),
                relation: "parallels".to_string(),
            });
        }
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieve::Passage;

    fn p(id: &str, r: &str, mv: &str) -> Passage {
        Passage {
            id: id.into(),
            ref_: r.into(),
            book_author: String::new(),
            text_original: String::new(),
            text_modern: String::new(),
            context: String::new(),
            location: String::new(),
            occasion: String::new(),
            move_: mv.into(),
            translation: String::new(),
            domains: Vec::new(),
            principles: Vec::new(),
            score: None,
        }
    }

    #[test]
    fn projects_topic_sayings_moves_and_parallels() {
        let passages = vec![p("1", "Mark 12:17", "M02"), p("2", "Luke 20:25", "M02")];
        let d = project_topic("Caesar", &passages);
        // topic + 2 sayings + 1 move node.
        assert_eq!(d.nodes.iter().filter(|n| n.kind == "topic").count(), 1);
        assert_eq!(d.nodes.iter().filter(|n| n.kind == "saying").count(), 2);
        assert_eq!(
            d.nodes
                .iter()
                .filter(|n| n.kind == "reasoning_move")
                .count(),
            1
        );
        // 2 matches + 2 uses_move + 1 parallels.
        assert_eq!(
            d.edges.iter().filter(|e| e.relation == "matches").count(),
            2
        );
        assert_eq!(
            d.edges.iter().filter(|e| e.relation == "uses_move").count(),
            2
        );
        assert_eq!(
            d.edges.iter().filter(|e| e.relation == "parallels").count(),
            1
        );
    }

    #[test]
    fn unannotated_sayings_have_no_move_layer() {
        let passages = vec![p("1", "Mark 3:1", ""), p("2", "Luke 4:1", "")];
        let d = project_topic("healing", &passages);
        assert_eq!(
            d.nodes
                .iter()
                .filter(|n| n.kind == "reasoning_move")
                .count(),
            0
        );
        assert!(d.edges.iter().all(|e| e.relation == "matches"));
    }
}
