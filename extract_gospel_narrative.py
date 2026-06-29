#!/usr/bin/env python3
"""Extract the NON-red-letter Gospel narrative (his deeds, settings, the dialogue around the
sayings) from the public-domain World English Bible (WEB) USFX, as a separate labeled corpus.

This is the THIRD corpus (gospel-context-kb): what the record shows he *did* and the context in
which he spoke — never his own words (those are the red-letter `saying` corpus). It is labeled
"what the record shows," distinct from his teaching, with the same citation discipline.

Method: the WEB USFX (eBible.org) tags his speech with <wj>...</wj>. The red-letter extractor keeps
the <wj> spans; this keeps the COMPLEMENT — the narrative text of each Gospel verse with the <wj>
spans removed. A verse is emitted when its remaining narrative text is substantial (so a bare
speech tag like "He said," is dropped, but "He stretched out his hand and touched him" is kept).

Attestation: each passage carries an `attestation`/`witnesses` field. Computing multiply-vs-single
attestation MECHANICALLY needs synoptic-parallel data, which does not yet exist in this repo (the
`parallels` graph is unpopulated). So attestation is left "single" here and is a documented
follow-up (gospel-context-kb automated-attestation v1) — see docs/FINDINGS.md.

Usage (stdlib only):
    python extract_gospel_narrative.py --out build/gospel_narrative.jsonl
    python extract_gospel_narrative.py --usfx engwebpusfx.xml --out build/gospel_narrative.jsonl
"""
from __future__ import annotations

import argparse
import io
import json
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

USFX_ZIP_URL = "https://ebible.org/Scriptures/engwebp_usfx.zip"

# The four Gospels: book code -> display name. (Acts/Revelation are excluded — not Gospel narrative.)
GOSPELS = {
    "MAT": "Matthew",
    "MRK": "Mark",
    "LUK": "Luke",
    "JHN": "John",
}

# Minimum narrative characters (after removing <wj>) for a verse to count as narrative, not a
# bare speech tag.
MIN_NARRATIVE_CHARS = 25

# Element subtrees whose text is NOT narrative: his words (<wj>) and editorial apparatus
# (footnotes, cross-references). Entering any of these excludes all of its descendant text; the
# text AFTER it (its tail) continues the verse and is kept.
EXCLUDE_TAGS = {"wj", "f", "x", "fe", "ef", "fig", "rq"}


def local(tag: str) -> str:
    return tag.split("}")[-1] if "}" in tag else tag


def get_usfx_bytes(args) -> bytes:
    if args.usfx:
        with open(args.usfx, "rb") as f:
            return f.read()
    print(f"Downloading {USFX_ZIP_URL} ...", file=sys.stderr)
    req = urllib.request.Request(
        USFX_ZIP_URL,
        headers={"User-Agent": "Mozilla/5.0 (compatible; gospel-narrative/1.0)"},
    )
    data = urllib.request.urlopen(req, timeout=120).read()
    zf = zipfile.ZipFile(io.BytesIO(data))
    xmlname = next(
        (n for n in zf.namelist() if "usfx" in n.lower() and n.lower().endswith(".xml")),
        next(n for n in zf.namelist() if n.lower().endswith(".xml")),
    )
    print(f"Extracting {xmlname}", file=sys.stderr)
    return zf.read(xmlname)


def extract_narrative(xml_bytes: bytes) -> list[dict]:
    """Walk the USFX tree and collect, per Gospel verse, the text NOT inside <wj>."""
    root = ET.fromstring(xml_bytes)
    # (book, ch, v) -> accumulated narrative text segments
    verses: dict[tuple[str, str, str], list[str]] = {}
    order: list[tuple[str, str, str]] = []
    state = {"book": None, "ch": None, "v": None}

    def emit(seg: str | None, excluded: bool):
        if excluded or not seg or not seg.strip():
            return
        key = (state["book"], state["ch"], state["v"])
        if key[0] not in GOSPELS or key[1] is None or key[2] is None:
            return
        if key not in verses:
            verses[key] = []
            order.append(key)
        verses[key].append(seg)

    def walk(elem, excluded: bool):
        tag = local(elem.tag)
        if tag == "book":
            state["book"] = (elem.get("id") or "").upper()
            state["ch"] = state["v"] = None
        elif tag == "c":
            state["ch"] = elem.get("id")
        elif tag == "v":
            state["v"] = elem.get("id")
        here = excluded or tag in EXCLUDE_TAGS
        emit(elem.text, here)  # element's own leading text
        for child in elem:
            walk(child, here)
            # a child's tail is part of THIS element's run (e.g. verse text after a closing <wj>),
            # so it shares THIS element's excluded state — not the grandparent's.
            emit(child.tail, here)

    walk(root, False)

    out = []
    n = 1
    for (book, ch, v) in order:
        text = " ".join(" ".join(verses[(book, ch, v)]).split()).strip()
        if len(text) < MIN_NARRATIVE_CHARS:
            continue
        out.append(
            {
                "id": f"gn-{n}",
                "ref": f"{GOSPELS[book]} {ch}:{v}",
                "text": text,
                "book": GOSPELS[book],
                # Attestation deferred (needs synoptic-parallel data); default single witness.
                "attestation": "single",
                "witnesses": [GOSPELS[book]],
            }
        )
        n += 1
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="Extract the Gospel narrative (non-red-letter) corpus")
    ap.add_argument("--out", default="build/gospel_narrative.jsonl", help="Output JSONL path")
    ap.add_argument("--usfx", help="Path to a local USFX .xml (else download the WEB zip)")
    args = ap.parse_args()

    passages = extract_narrative(get_usfx_bytes(args))
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        for p in passages:
            f.write(json.dumps(p, ensure_ascii=False) + "\n")
    print(f"Wrote {len(passages)} Gospel-narrative passages to {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
