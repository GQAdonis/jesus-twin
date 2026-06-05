# Jesus Digital Twin Assessment

Date: 2026-06-05

## Executive Verdict

You are doing well on the most important architectural decision: the project keeps truth, voice, and stance in separate layers. That is the right shape for a historically humble, non-proselytizing study aid.

The current state is not yet a working digital twin. It is a strong design plus a working corpus pipeline. The RAG corpus is usable now, but the style twin is blocked because the extracted corpus has no completed annotations for `Modern Rendering` or `Reasoning Move`.

Overall status: **conceptually strong, implementation early, annotation blocked**.

| Area | Status | Assessment |
|---|---:|---|
| Scope definition | Strong | The repo correctly defines the product as a study aid grounded in cited sayings, not a theological oracle. |
| Truth/voice separation | Strong | `README.md`, `ARCHITECTURE.md`, and `training_data_spec.md` repeatedly enforce retrieval for truth and fine-tune for style only. |
| Historical humility | Strong in design | `ALIGNMENT_AND_TUNING.md` explicitly acknowledges that the Gospels are faith-community sources, not neutral primary transcripts. |
| Corpus extraction | Working | `build/rag_corpus.jsonl` currently contains 927 retrievable verse rows. |
| Annotation | Blocked | `build_training_jsonl.py` reports 0 training-ready rows out of 927. |
| Runtime agent | Not built | The Rust service is specified, but the repo still has no implemented Rust agent crates. |
| Evaluation | Designed, not operational | Grounding, style, citation, and refusal evals are specified but cannot run meaningfully until annotations and serving exist. |
| Bias controls | Good starting point | Attestation tiers, interpretation flags, and refusal gates are specified, but not implemented or source-cited yet. |

## Evidence From The Repository

The project's core principle is clear and repeated: **retrieval owns truth; the adapter owns voice; the agent layer owns stance and honesty**. `README.md` says the system renders Jesus' recorded teachings in present-day English, preserves reasoning moves, and never fabricates sayings because answers are grounded in cited verses. `ARCHITECTURE.md` turns that into a service design: one core event stream, RAG retrieval, a coverage gate, and citations as first-class events.

The data shape is also sound. `training_data_spec.md` rejects synthetic free-form Q&A where a model invents "Jesus answers." Instead, the original WEB text becomes the user input and the human-checked modern rendering becomes the label. That makes the LoRA learn a transformation of a real line, not a doctrine generator.

The current pipeline result is the bottleneck. Running `python3 build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build` produced:

```text
rows seen ............ 927
RAG passages ......... 927  -> build/rag_corpus.jsonl
training-ready ....... 0  (927 awaiting annotation)
  SFT train .......... 0  -> build/sft_style.jsonl
  held-out eval ...... 0  -> build/eval_heldout.jsonl
```

That means the project can start building a retrieval-first study aid, but it cannot yet train or evaluate the style LoRA. The six records in `sample_training_data.jsonl` are useful examples, not enough training data.

## External Evidence From Firecrawl Research

The web research supports your design choices, but also tightens the warnings.

Adela Yarbro Collins' Yale essay on the historical Jesus emphasizes that the modern historical-Jesus problem is precisely the distinction between the historical Jesus and the Christ of faith. It also notes that historical-Jesus portraits vary by reconstructive lens, and that even strong historical portraits remain probabilistic reconstructions rather than direct access to Jesus himself. Source: https://reflections.yale.edu/article/between-babel-and-beatitude/historical-jesus-then-and-now

Bart Ehrman's discussion of historical criteria states that traditions about Jesus circulated orally, changed in transmission, and in some cases were made up. He treats criteria such as independent attestation and dissimilarity as useful but limited probability tools, not certainty machines. Source: https://ehrmanblog.org/jesus-and-the-historical-criteria/

The red-letter limitation is real. A Firecrawl-scraped article on red-letter Bibles notes that Greek manuscripts did not contain modern quotation marks, Jesus likely spoke Aramaic while the Gospels are Greek, and there is a decades-long gap between speech events and written Gospel form. Even a sympathetic Christian framing distinguishes exact words, `ipsissima verba`, from the approximate voice or gist, `ipsissima vox`. Source: https://sharedveracity.net/2015/05/07/are-jesus-words-really-in-red-letters/

Digital twin research also supports your RAG-first approach. Nielsen Norman Group's 2025 synthesis reports that digital twins work better when built on rich contextual data, remain susceptible to bias, and should complement rather than replace human-centered validation. It also notes that synthetic users often capture trends but not magnitude or variability. Source: https://www.nngroup.com/articles/ai-simulations-studies/

## Where The Project Is Strong

1. **You are not trying to make the model the source of truth.** This is the most important thing you are doing right. The model is allowed to paraphrase or render; it is not allowed to decide what Jesus said.

2. **You have the right failure mode.** A good version of this system should refuse or hedge. Your architecture already makes refusal-on-no-coverage a first-class behavior.

3. **You explicitly reject religious and anti-religious overreach.** `ALIGNMENT_AND_TUNING.md` frames the system as historical-critical, not devotional and not debunking. That is the correct neutrality target.

4. **You separate Jesus' words, Jesus' source material, Gospel narrative, and later interpretation.** The plan to use red-letter sayings as the core, Hebrew Bible as source material, Gospel narrative as attestation-flagged context, and Epistles/Acts/Revelation as excluded or quarantined later interpretation is methodologically strong.

5. **The event model makes honesty visible.** Custom chunks such as `CITATION`, `ATTESTATION`, `SOURCE_TEXT`, `REASONING_MOVE`, and `INTERPRETATION_FLAG` are exactly the right UI affordances. They let users see why the answer says what it says.

6. **The training record shape is protective.** Original text in, modern rendering out is much safer than persona Q&A. It teaches voice under constraint.

## Limitations And Risks

1. **The phrase "actual words spoken" overclaims the evidence.** The better wording is "recorded sayings attributed to Jesus in the Gospel tradition." Red-letter text is not a tape transcript. It is translated, transmitted, selected, and theologically edited.

2. **The current corpus is retrievable but not style-trainable.** The RAG path has 927 passages. The style path has 0 training-ready examples in the generated build artifacts.

3. **The annotation task is larger than it looks.** You need not only modern renderings, but consistent reasoning-move labels, context, audience, occasion, and eventually attestation tiers. Inconsistent annotations will teach a mushy voice and unreliable move mapping.

4. **A red-letter-only corpus can detach Jesus from his Jewish context.** The project already identifies the Hebrew Bible as his source material. That needs to become an implemented retrieval tool, not just a design note.

5. **Attestation tiering can smuggle in scholarly bias.** Multiple attestation, dissimilarity, embarrassment, and contextual credibility are useful, but contested. If you hardcode one scholar's framework as truth, you reintroduce bias under a neutral label.

6. **John's Gospel and synoptic sayings should not be flattened.** They differ in style, theology, and chronology. A single undifferentiated "Jesus voice" risks blending historically distinct Gospel portraits into an artificial composite.

7. **The term "digital twin" is aspirational.** Based on current digital twin research, a high-fidelity twin requires rich individual data. For Jesus, the available data is sparse, mediated, and not first-person autobiographical. The product can be an evidence-grounded simulation of recorded rhetorical patterns, but not a full psychological twin.

8. **The runtime safety story is still only designed.** The coverage gate, citation integrity, tool authorization, and adapter mappings need implementation before the main safety claims become true operationally.

9. **Evaluation is not optional.** Without adversarial refusal tests, citation checks, entailment checks, and per-move style evaluation, a fluent model can appear aligned while quietly inventing.

## Sycophancy Correction Findings

I used the sycophancy-correction lens while writing this assessment. The main risk in evaluating this project is overpraising the architecture because the intent is thoughtful. A sycophantic assessment would say the project is already close to producing a faithful Jesus twin. That would be false.

Detector result on this report draft: **0.0179 sycophancy score** under adversarial strictness. The only finding was low-severity `S-07`, meaning the report is long enough to warrant checking that its length serves analysis rather than visible effort. No correction was mandatory.

Corrected position:

| Claim to avoid | Better claim |
|---|---|
| "This can reconstruct Jesus' actual personality." | "This can model recurring rhetorical and reasoning patterns in recorded Gospel sayings, with explicit uncertainty." |
| "Red-letter text gives his actual words." | "Red-letter text gives translated Gospel attributions of his speech, with boundaries that must be treated probabilistically." |
| "The design solves bias." | "The design creates places to expose and manage bias, but implementation and source-cited attestation are still required." |
| "The LoRA will make Jesus' voice." | "The LoRA may improve modern rendering style if the annotation set is large, consistent, and evaluated against grounding." |

## Recommended Improvements

### 1. Rename the evidence target

Use this language throughout the docs and UI:

> "Recorded sayings attributed to Jesus in the Gospel tradition, rendered in present-day English with citations and confidence labels."

Avoid this language unless heavily qualified:

> "Jesus' actual words spoken."

This does not weaken the product. It makes it more honest.

### 2. Ship RAG before style

The fastest safe milestone is not the LoRA. It is a cited answer engine over `build/rag_corpus.jsonl` with refusal-on-no-coverage. That validates the product's truth layer before you add voice.

Minimum first milestone:

1. Load `build/rag_corpus.jsonl` into a searchable store.
2. Return top passages with `ref`, original WEB text, and score.
3. Add a hard refusal path for low coverage.
4. Require every answer to show citations.
5. Add adversarial prompts such as "What did Jesus say about cryptocurrency?" and verify refusal.

### 3. Build an annotation protocol before annotating everything

Do not just fill the spreadsheet row by row. First create a written annotation guide with examples and edge cases.

The guide should define:

1. How literal the modern rendering should be.
2. How much contemporary vocabulary is allowed.
3. How to handle divine-language terms without devotional expansion.
4. How to label each `M01-M18` move.
5. How to handle multi-move sayings.
6. How to treat synoptic parallels.
7. How to mark uncertain red-letter boundaries.

Then annotate 50 rows, run review, revise the guide, and only then scale.

### 4. Add attestation and source-critical metadata now

Extend the sheet or downstream metadata with fields like:

| Field | Purpose |
|---|---|
| `source_layer` | red-letter saying, Gospel narrative, Hebrew Bible source, later interpretation |
| `attestation_tier` | high, medium, low, disputed |
| `attestation_basis` | multiple attestation, contextual credibility, dissimilarity, editorial concern, disputed boundary |
| `gospel_family` | synoptic, Johannine, Acts/Revelation red-letter edge case |
| `confidence_note` | short human-readable caveat |
| `red_letter_boundary_confidence` | high, medium, low |

Make these labels revisable and source-cited. Do not make them a hidden truth table.

### 5. Separate synoptic and Johannine voice in evaluation

A single style score can hide major distortions. Evaluate at least these facets separately:

1. Synoptic aphorism and parable material.
2. Synoptic controversy dialogues.
3. Sermon/discourse material.
4. Johannine discourse material.
5. Passion sayings.
6. Post-resurrection sayings if included.

The goal is not to erase differences but to make them visible.

### 6. Treat `M01-M18` as an empirical claim, not just a rubric

The reasoning-move rubric is a strong idea. It should be tested.

Recommended checks:

1. Inter-annotator agreement on a 50-saying sample.
2. Confusion matrix for move labels.
3. Per-move retrieval recall.
4. Per-move style fidelity after any LoRA.
5. Human review of rare moves before oversampling.

### 7. Define product refusal behavior in exact text

The coverage gate should not improvise refusals. It should have standard refusal forms:

```text
The recorded sayings in this corpus do not show Jesus addressing that directly.
Closest related passages are: ...
```

```text
The source text is too weakly matched for me to answer in his voice. I can show related passages instead.
```

```text
That is later interpretation about Jesus, not a saying attributed to Jesus in this corpus.
```

### 8. Build a small evaluation suite before training

Create `eval/` artifacts before the Rust service is finished.

Minimum eval set:

1. 30 grounded rendering tests from annotated rows.
2. 30 retrieval tests with expected refs.
3. 30 refusal tests outside the corpus.
4. 20 interpretation-boundary tests, such as atonement, Trinity, resurrection meaning, Paul, and church doctrine.
5. 20 adversarial persona tests asking the twin to invent, bless, command, condemn, or act with authority.

### 9. Make the UI show uncertainty by default

The eventual UI should not just display an answer. It should display:

1. Source verse.
2. Original WEB text.
3. Modern rendering.
4. Attestation/confidence.
5. Reasoning move.
6. Interpretation flag.
7. Related synoptic parallels.

This is how the product earns trust without pretending certainty.

### 10. Reframe the product name if needed

"Digital twin of Jesus Christ" is emotionally powerful but technically risky. It implies stronger fidelity than the data can support. Safer public labels include:

1. "Jesus Sayings Study Twin"
2. "Historical Jesus Rhetoric Engine"
3. "Cited Jesus Sayings Companion"
4. "Red-Letter RAG Study Aid"
5. "Recorded Sayings Digital Twin"

If you keep "digital twin," define it narrowly in-product:

> "A constrained study-aid twin of the recorded sayings tradition, not a claim to direct access to the historical Jesus."

## Best Next 30 Days

1. **Write the annotation guide.** Include positive and negative examples for modern rendering, reasoning moves, and religious-neutral stance.
2. **Annotate 50 representative sayings.** Include synoptic controversy, parables, aphorisms, prayer, Johannine material, and passion sayings.
3. **Run `build_training_jsonl.py` and inspect the split.** Confirm nonzero SFT and eval output.
4. **Create the first retrieval-only prototype.** It can be Python or minimal Rust; the goal is cited retrieval plus refusal, not full model generation.
5. **Add source-critical metadata.** Even a first-pass `attestation_tier` and `confidence_note` will improve honesty.
6. **Create the eval suite.** Test refusal and citation integrity before testing voice.
7. **Only then train a small LoRA experiment.** Keep it if it improves style without weakening grounding.

## Bottom Line

The project is strongest where it is most skeptical of itself. The architecture correctly refuses to let a model become a source of doctrine, and the docs already recognize that the Gospel sources are mediated, translated, and theologically shaped. That is the right foundation.

The biggest improvement is to make the current design operational in the right order: retrieval and refusal first, annotation protocol second, limited style LoRA third, and broader agent surfaces last. If you keep that order, this can become a credible, evidence-grounded study aid. If you skip annotation discipline or let the model answer outside cited coverage, it will become exactly what the repo is trying to avoid: a fluent religious persona that sounds authoritative while exceeding the evidence.

## Sources Checked

Repository evidence:

1. `README.md`
2. `ARCHITECTURE.md`
3. `ALIGNMENT_AND_TUNING.md`
4. `training_data_spec.md`
5. `DATA_EXTRACTION.md`
6. `build_training_jsonl.py`
7. `sample_training_data.jsonl`

Firecrawl/web evidence:

1. Adela Yarbro Collins, "The Historical Jesus: Then and Now," Yale Reflections: https://reflections.yale.edu/article/between-babel-and-beatitude/historical-jesus-then-and-now
2. Bart Ehrman, "Jesus and the Historical Criteria": https://ehrmanblog.org/jesus-and-the-historical-criteria/
3. Clarke Morledge, "Are Jesus' Words Really in Red Letters?": https://sharedveracity.net/2015/05/07/are-jesus-words-really-in-red-letters/
4. Nielsen Norman Group, "Evaluating AI-Simulated Behavior: Insights from Three Studies on Digital Twins and Synthetic Users": https://www.nngroup.com/articles/ai-simulations-studies/
