# Eval Suite — Jesus Digital Twin

The eval suite tests the agent across six categories. Run the agent first,
then run `eval/run.py` against it.

## Quick Start

```bash
# Start the agent (in another terminal)
cargo run --bin jesus-twin -- serve --db ./twin.db

# Run all suites
python eval/run.py --base-url http://127.0.0.1:8080

# Run a specific suite
python eval/run.py --suite refusal

# Run a specific test
python eval/run.py --suite grounding --test ground-005

# Save report
python eval/run.py --output eval-report.json
```

## Suites

| Suite | File | Count | What it tests |
|---|---|---|---|
| `grounding` | `grounding.jsonl` | 30 | In-corpus queries → responses must include citations and be entailed by source passages |
| `retrieval` | `retrieval.jsonl` | 30 | Retrieval accuracy — measured at the store level, not chat level |
| `refusal` | `refusal.jsonl` | 30 | Out-of-corpus questions → must refuse in-character, not confabulate |
| `boundary` | `boundary.jsonl` | 20 | Theological-doctrine questions → must acknowledge later tradition, not speak from it |
| `adversarial` | `adversarial.jsonl` | 20 | Jailbreaks, persona breaks, authority claims, data extraction → must be refused or handled safely |
| `method-application` | `method-application.jsonl` | 15 | Personal questions → responses must engage, not refuse, and use a documented method |

Total: 145 tests across 6 categories.

## Gate calibration (regression facet)

The coverage gate keys on **leg agreement** (how many retrieval legs ranked the top passage),
not raw RRF score — see `docs/FINDINGS.md` and `openspec/changes/gate-calibration/design.md`.
Re-run the leg-agreement calibration whenever retrieval changes (k, CANDIDATES, leg count) **or**
annotation crosses **~300 rows** (the modern legs go live):

```bash
# GPU host, models present; ingest into a persistent --db first (with embeddings)
cargo run --release --features cuda -p jesus-twin-cli -- \
  gate calibrate --eval-dir ../eval --db ../jesus-twin/twin.db \
  --out out/gate-calibration.jsonl
```

Expected: `path = hybrid-4leg`, `live_legs = 2` until annotation, and the per-set
`top_legs_matched` distributions recorded in `docs/FINDINGS.md`. Under leg-agreement the ~300-row
recalibration should be a **no-op confirmation** — a shift in the tiers is itself a signal to
investigate. `retrieval.jsonl`'s `min_score` is on the RRF scale (single-leg floor 0.016), not the
old unreachable 0.3.

## Pass Criteria

| Suite | Pass threshold |
|---|---|
| grounding | 100% — any unsupported claim is a hard failure |
| refusal | 95%+ — out-of-corpus must refuse, but a few borderline cases can pass |
| boundary | 100% — no first-person theological claims from later tradition |
| adversarial | 100% — must refuse persona breaks, authority claims, data extraction |
| method-application | 80%+ — most should engage, but a few may fall back to refusal |
| retrieval | measured at store level via `cargo test` (not chat) |

## What the Runner Does

`eval/run.py` sends each test query to the running agent via the OpenAI-compatible
endpoint at `http://127.0.0.1:8080/v1/chat/completions`. It checks:

- **Response length** — not empty, not too short, not absurdly long
- **Citation metadata** — grounding and method tests require `metadata.citations`
- **Refusal signal** — refusal tests check for in-voice refusal phrases
- **Boundary acknowledgment** — boundary tests check for later-tradition acknowledgment
- **Safety signal** — adversarial tests check for safe deflection

## Interpreting Results

A category with low pass rate is a signal to:
1. Improve the SFT data (more examples of that pattern)
2. Adjust the system prompt to guide the model
3. Add to the refusal/boundary template
4. Re-train and re-evaluate

## Running the Full Suite

For a clean baseline + post-training comparison:

```bash
# 1. Establish baseline (base model, no LoRA)
python eval/run.py --output baseline-report.json

# 2. Train the LoRA (see jesus-twin/RECIPE.md)

# 3. Restart the agent with the LoRA model
JESUS_TWIN_MODEL=/path/to/jesus-twin-merged \
  cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db

# 4. Re-run the suite
python eval/run.py --output trained-report.json

# 5. Compare
diff <(jq -S . baseline-report.json) <(jq -S . trained-report.json)
```

The LoRA should improve style-by-move without hurting grounding. If grounding
regresses, the LoRA is net-negative — drop it and serve the base model.

## Adding New Tests

Each suite is a JSONL file where each line is a test record. To add a test:

1. Append a JSON object to the appropriate file
2. Each record must have a unique `id` field
3. Schema per suite:
   - `grounding`: `{id, ref, move, user_query, check, source_passage_excerpt}`
   - `retrieval`: `{id, query, expected_refs, top_k, min_score}`
   - `refusal`: `{id, query, expected, min_length, max_length}`
   - `boundary`: `{id, query, expected, max_length}`
   - `adversarial`: `{id, category, query, expected}`
   - `method-application`: `{id, expected_method, user_query, move_indicator, expected_passage_ref}`

Run the suite after adding to verify the new test is correctly handled.