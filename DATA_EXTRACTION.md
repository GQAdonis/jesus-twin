# Red-Letter Corpus Extractor

> Moved from the original `README.md` when the project was reframed as the Rust
> digital-twin build. This documents the data-extraction tooling, which remains the
> source of the annotated corpus the twin trains on. See `README.md` for the overall
> architecture.

Extract the complete red-letter corpus (words of Jesus) from the public-domain
[World English Bible (WEB)](https://ebible.org/) into a structured Excel
spreadsheet with a 12-column annotation schema.

## Quick Start

```bash
pip install openpyxl
python extract_red_letter_corpus.py --out jesus_full_red_letter.xlsx
```

The script downloads the WEB USFX zip from eBible.org, extracts every `<wj>`
(words of Jesus) tag from the six NT books that contain red-letter text
(Matthew, Mark, Luke, John, Acts, Revelation), groups consecutive verses into
contiguous sayings, and writes the result to an Excel file.

### Local XML

If you already have the USFX XML file on disk, skip the download:

```bash
python extract_red_letter_corpus.py --usfx engwebp_usfx.xml --out out.xlsx
```

### Self-Test

```bash
python extract_red_letter_corpus.py --selftest
```

## Output Schema

| Column | Description |
|--------|-------------|
| ID | Sequential saying number |
| Scripture | Book chapter:verse reference |
| Author of Book | Traditional author attribution |
| Original (WEB) | Full WEB text of the saying |
| Modern Rendering | *(blank — for annotation)* |
| Situational Context | *(blank — for annotation)* |
| Sentiment | *(blank — for annotation)* |
| Audience Present | *(blank — for annotation)* |
| Approx. Age | *(blank — for annotation)* |
| Location | *(blank — for annotation)* |
| Reason Present / Occasion | *(blank — for annotation)* |
| Reasoning Move | *(blank — for annotation)* |

## Known Issues & Fixes

- **HTTP 403 on download**: eBible.org rejects requests with Python's default
  `urllib` User-Agent. The script sends a custom `User-Agent` header to avoid
  this.
- **Wrong XML extracted from zip**: The USFX zip contains several XML files
  (e.g. `BookNames.xml`). The script now preferentially selects the file
  containing `usfx` in its name to ensure the correct scripture content is
  parsed.

## Files

- `extract_red_letter_corpus.py` — extraction script
- `jesus_sayings_dataset.xlsx` — schema reference / seed data
- `jesus_full_red_letter.xlsx` — generated output (489 sayings)

## License

The World English Bible is in the public domain.
