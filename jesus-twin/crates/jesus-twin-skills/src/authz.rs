//! Deterministic tool authorization (ALIGNMENT_AND_TUNING.md §3).
//!
//! The character layer proposes; this policy layer disposes — independently of the persona,
//! so a jailbreak of the voice can't escalate privileges. Reads run autonomously; outbound
//! or irreversible actions must pass an explicit gate. This is a *security boundary*, not a
//! prompt: it is enforced in code, with an audit record, before any skill executes.

use crate::skill::RiskClass;

/// The outcome of an authorization check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Cleared to execute.
    Allow,
    /// Blocked, with a reason for the audit trail / user message.
    Deny(String),
}

/// Decides whether a skill of a given risk class may run. Implementations are deterministic
/// per their inputs (no model in the loop).
pub trait Authorizer: Send + Sync {
    fn authorize(&self, skill: &str, risk: RiskClass) -> Decision;
}

/// The safe default for the study-aid twin: only [`RiskClass::ReadOnly`] skills run; anything
/// that acts on the outside world is denied outright (no human present to approve). This is
/// the right policy whenever the deployment has no approval channel wired.
#[derive(Debug, Default, Clone)]
pub struct AutoAllowReadOnly;

impl Authorizer for AutoAllowReadOnly {
    fn authorize(&self, skill: &str, risk: RiskClass) -> Decision {
        match risk {
            RiskClass::ReadOnly => Decision::Allow,
            RiskClass::Outbound | RiskClass::Irreversible => Decision::Deny(format!(
                "skill '{skill}' is {risk:?} and no approval channel is configured"
            )),
        }
    }
}

/// Human-in-the-loop authorization: reads run autonomously; [`RiskClass::Outbound`] and
/// [`RiskClass::Irreversible`] skills are routed to a `prompt` callback that returns the
/// human's approve/deny decision. The approval step is itself a prompt-injection mitigation —
/// an injected instruction that can't execute without approval can't silently act.
pub struct HumanCheckpoint<F>
where
    F: Fn(&str, RiskClass) -> bool + Send + Sync,
{
    prompt: F,
}

impl<F> HumanCheckpoint<F>
where
    F: Fn(&str, RiskClass) -> bool + Send + Sync,
{
    /// `prompt(skill_name, risk) -> approved`. The closure is the human checkpoint; it must be
    /// the genuine approval channel (a CLI confirm, an out-of-band approval, etc.).
    pub fn new(prompt: F) -> Self {
        Self { prompt }
    }
}

impl<F> Authorizer for HumanCheckpoint<F>
where
    F: Fn(&str, RiskClass) -> bool + Send + Sync,
{
    fn authorize(&self, skill: &str, risk: RiskClass) -> Decision {
        match risk {
            RiskClass::ReadOnly => Decision::Allow,
            other => {
                if (self.prompt)(skill, other) {
                    Decision::Allow
                } else {
                    Decision::Deny(format!("human declined the {other:?} skill '{skill}'"))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_default_allows_reads_denies_the_rest() {
        let az = AutoAllowReadOnly;
        assert_eq!(az.authorize("lookup", RiskClass::ReadOnly), Decision::Allow);
        assert!(matches!(
            az.authorize("send", RiskClass::Outbound),
            Decision::Deny(_)
        ));
        assert!(matches!(
            az.authorize("rm", RiskClass::Irreversible),
            Decision::Deny(_)
        ));
    }

    #[test]
    fn human_checkpoint_routes_risky_to_the_prompt() {
        let approve_all = HumanCheckpoint::new(|_, _| true);
        assert_eq!(
            approve_all.authorize("send", RiskClass::Outbound),
            Decision::Allow
        );

        let deny_all = HumanCheckpoint::new(|_, _| false);
        assert!(matches!(
            deny_all.authorize("send", RiskClass::Outbound),
            Decision::Deny(_)
        ));
        // Reads never reach the prompt.
        assert_eq!(
            deny_all.authorize("lookup", RiskClass::ReadOnly),
            Decision::Allow
        );
    }
}
