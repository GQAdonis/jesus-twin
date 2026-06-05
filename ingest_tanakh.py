#!/usr/bin/env python3
"""Ingest the Hebrew Bible (Tanakh) as a retrieval tool for *remez* allusions.

The Tanakh is Jesus' intellectual furniture — what he quoted, alluded to, and
reasoned from. Making it a retrieval tool lets the agent engage questions by
referencing what he himself would have cited, without claiming these as his words.

This is *not* used for persona training. It is a separate retrieval path,
clearly labeled in the agent's responses.

Source: JPS 1917 English Translation (public domain)
https://www.sacred-texts.com/bib/jps/

Usage:
    pip install openpyxl requests beautifulsoup4 lxml
    python ingest_tanakh.py --out build/tanakh.jsonl
"""
from __future__ import annotations

import argparse
import json
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


def fetch_jps_1917(book_name: str) -> str | None:
    """Fetch a book of the JPS 1917 Tanakh from sacred-texts.com.

    This is a placeholder — actual fetching requires HTML parsing of
    the book/chapter/verse structure. The full implementation is
    deferred to a follow-up PR.

    Returns the raw text or None if fetch fails.
    """
    # The JPS 1917 is at: https://www.sacred-texts.com/bib/jps/
    # Each book has a page like: https://www.sacred-texts.com/bib/jps/jps_gen.htm
    # We use a simple HTML-to-text extraction.
    try:
        import requests
        from bs4 import BeautifulSoup

        # Map book names to sacred-texts URL slugs
        url_slug = book_name.lower().replace(" ", "")
        if url_slug.startswith("i") or url_slug.startswith("ii") or url_slug.startswith("iii"):
            # I Samuel, II Kings, etc.
            roman = url_slug[:2] if url_slug.startswith("ii") else url_slug[:1]
            base = url_slug[2:]
            url = f"https://www.sacred-texts.com/bib/jps/jps_{base}{roman.lower()}.htm"
        else:
            url = f"https://www.sacred-texts.com/bib/jps/jps_{url_slug[:3]}.htm"

        resp = requests.get(url, timeout=30, headers={"User-Agent": "jesus-twin/1.0"})
        if resp.status_code != 200:
            return None

        soup = BeautifulSoup(resp.text, "lxml")
        # Remove navigation and headers
        for tag in soup(["script", "style", "nav", "header", "footer"]):
            tag.decompose()
        text = soup.get_text(separator="\n", strip=True)
        return text
    except Exception as e:
        print(f"WARN: failed to fetch {book_name}: {e}")
        return None


def parse_into_passages(raw_text: str, book_name: str, category: str) -> list[dict]:
    """Parse the raw text into structured passages.

    The sacred-texts format is roughly: chapter headings, then verse-by-verse text.
    We split on common patterns and produce {ref, text, book, category} records.
    """
    if not raw_text:
        return []
    passages = []
    # Heuristic split: each verse is roughly a paragraph.
    # Real implementation would parse the HTML structure for verse numbers.
    lines = [l.strip() for l in raw_text.split("\n") if l.strip()]
    chapter = 1
    verse = 1
    for line in lines:
        if len(line) < 10:
            # Likely a header or chapter marker
            if line.isdigit():
                chapter = int(line)
                verse = 1
            continue
        if len(line) > 500:
            # Long line — likely multiple verses; split on common verse markers
            for segment in line.split("; "):
                if len(segment) > 20:
                    passages.append({
                        "ref": f"{book_name} {chapter}:{verse}",
                        "text": segment,
                        "book": book_name,
                        "category": category,
                        "translation": "JPS 1917",
                    })
                    verse += 1
        else:
            passages.append({
                "ref": f"{book_name} {chapter}:{verse}",
                "text": line,
                "book": book_name,
                "category": category,
                "translation": "JPS 1917",
            })
            verse += 1
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
        raw = fetch_jps_1917(book)
        if not raw:
            print("FAILED")
            continue
        passages = parse_into_passages(raw, book, category)
        all_passages.extend(passages)
        print(f"OK ({len(passages)} passages)")

    with open(out_path, "w") as f:
        for p in all_passages:
            f.write(json.dumps(p, ensure_ascii=False) + "\n")
    print(f"\n✓ Wrote {len(all_passages)} passages to {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
