# AGENTS.md

Project-level agent instructions. Read by **Zed**'s AI (`AGENTS.md` is a native root rule
file), **Codex**, and any other agent that honors the open `AGENTS.md` standard. Claude Code
also reads `CLAUDE.md`; OpenCode, Codex, and Claude additionally have generated OpenSpec
command/skill integrations in `.opencode/`, `.codex/`, and `.claude/`.

## Project

A Rust "Jesus digital twin" agent service (`jesus-twin/`) — see `CLAUDE.md` for the full
engineering rules and architecture, and `jesus-twin/README.md` for the workspace. The
governing principle: **retrieval owns truth, the fine-tune owns voice, the agent layer owns
stance/honesty** — the model never invents; its worst case is a paraphrase of a cited verse.

## Spec-driven development with OpenSpec

This repo uses **OpenSpec** (CLI: `openspec`, schema: `spec-driven`) for any non-trivial
feature, change, or refactor. Specs live in `openspec/specs/`; in-flight proposals live in
`openspec/changes/`. **Propose a change before writing code** for anything beyond a trivial
fix.

### Workflow (use the `openspec` CLI directly when no slash command is available)

1. **Explore / propose** — start a change for a new feature/fix/refactor:
   `openspec new` (or, in Claude/OpenCode/Codex, the `/opsx:new` command / `openspec-new-change`
   skill). For thinking through an idea first, use `openspec` explore.
2. **Draft artifacts** — generate the change's artifacts (proposal, specs, tasks). Fast-forward
   all at once with `openspec change` fast-forward, or step through with continue.
3. **Implement** — apply the tasks (`/opsx:apply` / `openspec-apply-change`). Make atomic
   commits; keep changes surgical (per `CLAUDE.md`).
4. **Verify** — confirm the implementation matches the artifacts (`/opsx:verify`) before
   archiving.
5. **Archive** — finalize and fold the change's deltas into the main specs
   (`openspec archive <change>` / `/opsx:archive`).

### Useful commands

```bash
openspec list                 # list in-flight changes
openspec list --specs         # list specs
openspec view                 # interactive dashboard
openspec show <item>          # show a change or spec
openspec validate [item]      # validate changes/specs
openspec status               # artifact completion status for a change
openspec archive <change>     # archive a completed change
```

If the `openspec` CLI is missing, install it (Homebrew: `brew install openspec`, or `npx
openspec`) before running the workflow.

## Engineering rules (summary — full set in `CLAUDE.md`)

- **Think before coding; surgical changes; simplicity first.** Don't refactor unrelated code.
- **Feature-based clean architecture**; no source file over 500 lines.
- **Rust:** run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` before
  considering work done; keep all tests green (`cargo test`). The workspace is `jesus-twin/`.
- **Evidence before claims** — run the verification commands and quote real output; never assert
  success without it.
- **Consult memory / validate new architecture** against current best practices (web search)
  before adopting it.
