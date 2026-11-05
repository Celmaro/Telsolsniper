#!/usr/bin/env python3
"""Analyze captured channel messages: which surfaces carry the mint, and
what the message templates look like.

Reads data/captures/*.jsonl produced by scripts/capture-channels.mjs.
Usage: python3 scripts/analyze-captures.py
"""
import glob
import json
import re
import statistics
from collections import Counter, defaultdict
from urllib.parse import urlparse, parse_qs

BASE58 = re.compile(r"\b[1-9A-HJ-NP-Za-km-z]{32,44}\b")
CAPTURE_DIR = "data/captures"


def load():
    records = []
    for path in sorted(glob.glob(f"{CAPTURE_DIR}/*.jsonl")):
        channel = path.split("/")[-1].replace(".jsonl", "")
        for line in open(path):
            line = line.strip()
            if not line:
                continue
            record = json.loads(line)
            record["_channel"] = channel
            records.append(record)
    return records


def classify_url(url):
    """Return (host, path-template, mint-source-kind) for a URL."""
    try:
        parsed = urlparse(url)
    except Exception:
        return ("invalid", "", "none")
    host = parsed.hostname or "invalid"
    host = host.replace("www.", "")
    path = parsed.path
    query = parse_qs(parsed.query)
    mint = None
    kind = "none"

    # path-embedded mint
    match = re.search(r"/(?:token|coin)/([1-9A-HJ-NP-Za-km-z]{32,44})", path)
    if match:
        mint, kind = match.group(1), "path"
    elif "start" in query:
        start = query["start"][0]
        m = re.search(r"([1-9A-HJ-NP-Za-km-z]{32,44})", start)
        if m:
            mint, kind = m.group(1), "start-param"
        prefix = re.match(r"([a-z]+_?)", start)
        return (host, f"/{prefix.group(1) if prefix else ''}…", "start-param" if mint else "start-no-mint")

    template = re.sub(r"[1-9A-HJ-NP-Za-km-z]{32,44}", "<MINT>", path) or "/"
    return (host, template, kind if mint else "no-mint")


def main():
    records = load()
    print("=" * 74)
    print(f"CAPTURE ANALYSIS — {len(records)} messages")
    print("=" * 74)

    by_channel = Counter(r["_channel"] for r in records)
    print("messages per channel:", dict(by_channel))
    print()

    # 1. Which surface actually carries a mint?
    surface_hits = Counter()
    per_channel = defaultdict(Counter)
    for r in records:
        surfaces = r["surfaces"]
        button_mint = any(
            re.search(r"[1-9A-HJ-NP-Za-km-z]{32,44}", u) for u in surfaces["buttonUrls"]
        )
        hidden_mint = any(
            re.search(r"[1-9A-HJ-NP-Za-km-z]{32,44}", u) for u in surfaces["hiddenUrls"]
        )
        entity_mint = any(
            re.search(r"[1-9A-HJ-NP-Za-km-z]{32,44}", u) for u in surfaces["entityUrls"]
        )
        cb = r["mints"]["fromCallback"]
        txt = r["mints"]["fromText"]
        if button_mint:
            surface_hits["button_url"] += 1
            per_channel[r["_channel"]]["button_url"] += 1
        if hidden_mint:
            surface_hits["hidden_link"] += 1
            per_channel[r["_channel"]]["hidden_link"] += 1
        if entity_mint:
            surface_hits["entity_url"] += 1
            per_channel[r["_channel"]]["entity_url"] += 1
        if cb:
            surface_hits["callback_data"] += 1
            per_channel[r["_channel"]]["callback_data"] += 1
        if txt:
            surface_hits["plain_text"] += 1
            per_channel[r["_channel"]]["plain_text"] += 1
        if not (button_mint or hidden_mint or entity_mint or cb or txt):
            surface_hits["no_mint_anywhere"] += 1
            per_channel[r["_channel"]]["no_mint_anywhere"] += 1

    print("SURFACE THAT CARRIES A MINT (messages, may overlap)")
    for name, count in surface_hits.most_common():
        print(f"  {name:<18} {count:>4}  ({100*count/len(records):.0f}%)")
    print()

    print("PER CHANNEL")
    for channel in sorted(per_channel):
        parts = ", ".join(f"{k}={v}" for k, v in per_channel[channel].most_common())
        print(f"  {channel:<20} {parts}")
    print()

    # 2. URL templates
    print("URL TEMPLATES (host + path)")
    templates = Counter()
    for r in records:
        for url in (
            r["surfaces"]["entityUrls"]
            + r["surfaces"]["hiddenUrls"]
            + r["surfaces"]["buttonUrls"]
        ):
            host, template, kind = classify_url(url)
            templates[(host, template, kind)] += 1
    for (host, template, kind), count in templates.most_common(18):
        print(f"  {count:>4}x  {host:<26} {template:<28} [{kind}]")
    print()

    # 3. Mint suffixes (protocol fingerprint)
    suffixes = Counter()
    for r in records:
        for mint in (
            r["mints"]["fromUrls"] + r["mints"]["fromCallback"] + r["mints"]["fromText"]
        ):
            if mint.endswith("pump"):
                suffixes["…pump"] += 1
            elif mint.endswith("high"):
                suffixes["…high"] += 1
            elif mint.endswith("Loat"):
                suffixes["…Loat"] += 1
            else:
                suffixes[f"…{mint[-4:]}"] += 1
    print("MINT SUFFIXES (launchpad fingerprint)")
    for suffix, count in suffixes.most_common(8):
        print(f"  {suffix:<8} {count}")
    print()

    # 4. Mints per message — one post can contain a whole list
    counts = [len(r["mints"]["fromUrls"] + r["mints"]["fromCallback"] + r["mints"]["fromText"]) for r in records]
    multi = [c for c in counts if c > 1]
    print("MINTS PER MESSAGE")
    print(f"  0 mints: {counts.count(0)}   1 mint: {counts.count(1)}   >1 mint: {len(multi)}")
    if multi:
        print(f"  max {max(multi)} mints in one post, median {statistics.median(multi):.0f}")
    print()

    # 5. Message templates (first line shape)
    print("MESSAGE TEMPLATES (first line, mint-stripped)")
    shapes = Counter()
    for r in records:
        first = (r["text"] or "").strip().split("\n")[0][:60]
        shape = re.sub(r"[1-9A-HJ-NP-Za-km-z]{32,44}", "<MINT>", first)
        shape = re.sub(r"\d+", "N", shape)
        shapes[shape or "(empty)"] += 1
    for shape, count in shapes.most_common(10):
        print(f"  {count:>4}x  {shape}")
    print()

    # 6. Ticker presence
    ticker_bearing = sum(1 for r in records if "$" in (r["text"] or ""))
    print(f"MESSAGES CONTAINING A $TICKER: {ticker_bearing} ({100*ticker_bearing/len(records):.0f}%)")

    # 7. Empty-text messages (media only?)
    empty = [r for r in records if not (r["text"] or "").strip()]
    print(f"EMPTY-TEXT MESSAGES: {len(empty)}  (media-only or service messages)")
    if empty:
        with_media = sum(1 for r in empty if r.get("hasMedia"))
        print(f"  of those, with media: {with_media}")


if __name__ == "__main__":
    main()
