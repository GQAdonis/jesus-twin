#!/usr/bin/env python3
"""Ingest the Hebrew Bible (Tanakh) as a retrieval tool for *remez* allusions.

The Tanakh is Jesus' intellectual furniture — what he quoted, alluded to, and
reasoned from. Making it a retrieval tool lets the agent engage questions by
referencing what he himself would have cited, without claiming these as his words.

This is *not* used for persona training. It is a separate retrieval path,
clearly labeled in the agent's responses.

Source: JPS 1917 English Translation (public domain), served verse-accurate by the Sefaria
API (the exact version "The Holy Scriptures: A New Translation (JPS 1917)", license
"Public Domain" — NOT Sefaria's default modern RJPS, which is CC-BY-NC).

Usage (stdlib only — no third-party deps):
    python ingest_tanakh.py --out build/tanakh.jsonl              # full Tanakh (~23k verses)
    python ingest_tanakh.py --out build/tanakh.sample.jsonl --limit 2   # first 2 books (sample)
    python ingest_tanakh.py --dry-run
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

# Tanakh books in traditional Jewish order
TANAKH_BOOKS = [
    # Torah (5)
    ("Genesis", "torah"),
    ("Exodus", "torah"),
    ("Leviticus", "torah"),
    ("Numbers", "torah"),
    ("Deuteronomy", "torah"),
    # Nevi'im / Prophets (8)
    ("Joshua", "prophets"),
    ("Judges", "prophets"),
    ("I Samuel", "prophets"),
    ("II Samuel", "prophets"),
    ("I Kings", "prophets"),
    ("II Kings", "prophets"),
    ("Isaiah", "prophets"),
    ("Jeremiah", "prophets"),
    ("Ezekiel", "prophets"),
    # (The Twelve are often listed together; we'll handle them separately if needed)
    ("Hosea", "prophets"),
    ("Joel", "prophets"),
    ("Amos", "prophets"),
    ("Obadiah", "prophets"),
    ("Jonah", "prophets"),
    ("Micah", "prophets"),
    ("Nahum", "prophets"),
    ("Habakkuk", "prophets"),
    ("Zephaniah", "prophets"),
    ("Haggai", "prophets"),
    ("Zechariah", "prophets"),
    ("Malachi", "prophets"),
    # Ketuvim / Writings (11)
    ("Psalms", "writings"),
    ("Proverbs", "writings"),
    ("Job", "writings"),
    ("Song of Songs", "writings"),
    ("Ruth", "writings"),
    ("Lamentations", "writings"),
    ("Ecclesiastes", "writings"),
    ("Esther", "writings"),
    ("Daniel", "writings"),
    ("Ezra", "writings"),
    ("Nehemiah", "writings"),
    ("I Chronicles", "writings"),
    ("II Chronicles", "writings"),
]


# The public-domain JPS 1917 version, served verse-accurate by Sefaria's API. (Sefaria's
# DEFAULT English text is the modern RJPS — CC-BY-NC, NOT public domain — so we MUST pin this
# exact version title via `ven=`.)
JPS_1917 = "The Holy Scriptures: A New Translation (JPS 1917)"
SEFARIA_API = "https://www.sefaria.org/api/texts"

_TAG_RE = re.compile(r"<[^>]+>")
_WS_RE = re.compile(r"\s+")


def _clean(s: str) -> str:
    """Strip Sefaria's HTML footnote/markup and normalize whitespace."""
    return _WS_RE.sub(" ", _TAG_RE.sub("", s)).replace(" ", " ").strip()


def fetch_book(book_name: str, category: str) -> list[dict]:
    """Fetch one Tanakh book (JPS 1917) from Sefaria as verse-accurate passage records.

    One request returns the whole book as nested chapters→verses, so refs are exact
    (`Book chapter:verse`). Returns `{ref, text, book, category, translation}` records;
    empty list on failure (caller logs and continues)."""
    import urllib.parse
    import urllib.request

    url = (
        f"{SEFARIA_API}/{urllib.parse.quote(book_name)}"
        f"?ven={urllib.parse.quote(JPS_1917)}&context=0&commentary=0&pad=0"
    )
    req = urllib.request.Request(url, headers={"User-Agent": "jesus-twin/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.load(resp)
    except Exception as e:  # noqa: BLE001 — network/parse; log and skip the book
        print(f"WARN: failed to fetch {book_name}: {e}")
        return []

    if data.get("license") != "Public Domain" or "1917" not in (data.get("versionTitle") or ""):
        print(f"WARN: {book_name}: unexpected version/license "
              f"({data.get('versionTitle')!r}/{data.get('license')!r}) — skipping")
        return []

    chapters = data.get("text") or []
    passages: list[dict] = []
    for c_idx, chapter in enumerate(chapters, start=1):
        # A book is list[chapter]; a chapter is list[verse]. Guard against flat shapes.
        verses = chapter if isinstance(chapter, list) else [chapter]
        for v_idx, verse in enumerate(verses, start=1):
            text = _clean(verse if isinstance(verse, str) else " ".join(verse))
            if not text:
                continue
            passages.append({
                "ref": f"{book_name} {c_idx}:{v_idx}",
                "text": text,
                "book": book_name,
                "category": category,
                "translation": "JPS 1917",
            })
    return passages


def main() -> int:
    ap = argparse.ArgumentParser(description="Ingest the Tanakh (JPS 1917) for *remez* retrieval")
    ap.add_argument("--out", default="build/tanakh.jsonl", help="Output JSONL path")
    ap.add_argument("--limit", type=int, default=0, help="Limit number of books (0 = all)")
    ap.add_argument("--dry-run", action="store_true", help="Don't fetch; just print plan")
    args = ap.parse_args()

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    print(f"Plan: ingest {len(TANAKH_BOOKS)} Tanakh books")
    if args.dry_run:
        for book, category in TANAKH_BOOKS:
            print(f"  - {book} ({category})")
        return 0

    all_passages = []
    for i, (book, category) in enumerate(TANAKH_BOOKS):
        if args.limit and i >= args.limit:
            break
        print(f"[{i+1}/{len(TANAKH_BOOKS)}] {book} ({category})...", end=" ", flush=True)
        passages = fetch_book(book, category)
        if not passages:
            print("FAILED")
            continue
        all_passages.extend(passages)
        print(f"OK ({len(passages)} verses)")

    with open(out_path, "w") as f:
        for p in all_passages:
            f.write(json.dumps(p, ensure_ascii=False) + "\n")
    print(f"\n✓ Wrote {len(all_passages)} passages to {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
