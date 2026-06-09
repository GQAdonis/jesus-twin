# Spec delta: grounded-generation

How retrieved passages are framed to the generation model so the mentor speaks *from* them
rather than treating them as user-submitted material.

## ADDED Requirements

### Requirement: Retrieved context MUST be labeled with its provenance

The generation prompt SHALL frame retrieved passages as the mentor's own attested recall, not
as material the user supplied. The model SHALL NOT attribute retrieved passages to the person
asking the question.

#### Scenario: Passages are framed as the mentor's own recall
- **WHEN** the orchestrator assembles retrieved passages into the generation request
- **THEN** the passage block is preceded by an instruction line stating the passages are
  drawn from the mentor's attested teachings and that the person asking has not seen them
- **AND** the system prompt declares that provided passages are the mentor's own grounding
  material, not a user submission

#### Scenario: The answer does not attribute retrieval to the user
- **WHEN** a grounded answer is generated for an in-corpus question
- **THEN** the answer does not say the user "presented", "showed", or "gave" the scriptures
- **AND** the answer speaks directly to the person as their mentor

### Requirement: The handling instruction MUST be placed at the end of the user turn

The provenance-and-handling instruction SHALL be positioned at the end of the assembled user
turn, immediately before the passage block, rather than buried before the question — so it
occupies the high-attention end-of-prompt position (Lost-in-the-Middle, Liu et al. 2023).

#### Scenario: Question precedes the labeled passage block
- **WHEN** the inference layer assembles the user message from `context` and `user`
- **THEN** the user's question appears before the labeled passage block
- **AND** the labeled passage block is the final segment of the user turn

### Requirement: System-prompt provenance clause MUST stay train/inference identical

The added system-prompt clause SHALL be byte-identical across every copy of the prompt
(`prompt.rs`, `build_training_jsonl.py`, `ollama/Modelfile.jesus-twin`, `PROMPTS.md`), per the
existing prompt-parity invariant, so served behavior never drifts from trained behavior.

#### Scenario: All prompt copies match
- **WHEN** the SYSTEM_PROMPT is changed in any one location
- **THEN** the same change is present byte-for-byte in all parity copies
