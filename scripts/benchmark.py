#!/usr/bin/env python3
"""Reproducible acceptance + separate build, process and generation timings."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--receipts", type=Path, required=True)
parser.add_argument("--iterations", type=int, default=100)
parser.add_argument("--samples", type=int, default=5)
parser.add_argument("--synthetic-only", action="store_true", help="Explicitly benchmark only public synthetic cases")
args = parser.parse_args()
if not args.synthetic_only and not os.environ.get("BENDEC_SCHEMA_ROOT"):
    parser.error("full corpus requires BENDEC_SCHEMA_ROOT; alternatively pass --synthetic-only")
assert args.iterations > 0 and args.samples > 0
args.receipts.mkdir(parents=True, exist_ok=True)


def run(command, name, env=None):
    start = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    wall = time.monotonic() - start
    (args.receipts / f"{name}.stdout").write_text(result.stdout)
    (args.receipts / f"{name}.stderr").write_text(result.stderr)
    if result.returncode:
        raise RuntimeError(f"{name} failed ({result.returncode}); see {args.receipts}")
    return result.stdout, wall


_, build_wall = run(["cargo", "build", "--locked", "--release", "--bins", "--examples", "--target-dir", "target"], "build")
compat_command = ["node", "scripts/compatibility.cjs"]
if args.synthetic_only:
    compat_command.append("--synthetic-only")
compat, compat_wall = run(compat_command, "compatibility", {**os.environ, "BENDEC_RECEIPTS": str(args.receipts.resolve())})
corpus = args.receipts.resolve() / "corpus.json"
samples = {"rust": [], "javascript": []}
for index in range(args.samples):
    commands = {
        "rust": [str(ROOT / "target/release/examples/measure"), str(corpus), str(args.iterations)],
        "javascript": ["node", "scripts/measure-js.cjs", str(corpus), str(args.iterations)],
    }
    for language in (["rust", "javascript"] if index % 2 == 0 else ["javascript", "rust"]):
        text, wall = run(commands[language], f"{language}-{index}")
        samples[language].append({**json.loads(text), "process_wall_s": wall})
all_samples = samples["rust"] + samples["javascript"]
assert len({sample["bytes_per_iteration"] for sample in all_samples}) == 1
medians = {language: statistics.median(s["generation_us"] / s["iterations"] for s in runs) for language, runs in samples.items()}
report = {
    "platform": platform.platform(),
    "corpus_sha256": hashlib.sha256(corpus.read_bytes()).hexdigest(),
    "build_wall_s": build_wall,
    "compatibility_harness_wall_s": compat_wall,
    "compatibility": json.loads(compat.splitlines()[-1]),
    "samples": samples,
    "median_generation_us_per_corpus": medians,
    "warmed_generation_speedup": medians["javascript"] / medians["rust"],
}
(args.receipts / "benchmark.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
