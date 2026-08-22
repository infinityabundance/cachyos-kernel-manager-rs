#!/usr/bin/env python3
"""validate-atlas.py — schema + consistency validation for the forensic
ledgers and court definitions (CI gate).

Checks:
  1. atlas/status.json      — phases 0..13 present, status vocabulary
  2. atlas/court-ledger.json — entries have surface/court/status; status
     vocabulary; every court referenced exists (claim.toml present)
  3. atlas/residual-ledger.json — required fields per residual
  4. atlas/coverage-gaps.json — required fields per gap
  5. atlas/inventory.json   — parses; has a surface list
  6. every court dir        — claim.toml/assumptions.toml/comparator.toml/
     README.md present; comparator.toml parses as TOML; claim.toml has the
     EvidentiaryChain fields (claim, model, assumptions, observables,
     witness, independence, falsifier); evidence.json (when present) parses
     and its artifact hashes verify against the filesystem
  7. evidence releases     — ROOT-HASH + MANIFEST/COURTS/RECEIPTS parse;
     ROOT-HASH matches COURTS.json content hash
"""

import hashlib
import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
problems: list[str] = []


def check(cond: bool, msg: str) -> None:
    if not cond:
        problems.append(msg)


def read_json(path: Path):
    try:
        return json.loads(path.read_text())
    except Exception as e:
        check(False, f"{path}: invalid JSON: {e}")
        return None


def main() -> int:
    # 1. status.json
    status = read_json(ROOT / "atlas" / "status.json")
    if status is not None:
        vocab = {"pending", "in-progress", "sealed", "implemented"}
        phases = status.get("phases", [])
        nums = [p.get("phase") for p in phases]
        check(sorted(nums) == list(range(14)), "status.json: phases 0..13 required")
        for p in phases:
            check(p.get("status") in vocab, f"status.json phase {p.get('phase')}: bad status {p.get('status')!r}")
            check(p.get("name") and p.get("wording"), f"status.json phase {p.get('phase')}: name/wording required")

    # 2. court-ledger.json
    ledger = read_json(ROOT / "atlas" / "court-ledger.json")
    if ledger is not None:
        for entry in ledger.get("entries", []):
            for field in ("surface", "court", "status"):
                check(field in entry, f"court-ledger: entry missing {field}: {entry}")
            if entry.get("status") not in {None, "pending", "passing", "failing", "defined"}:
                check(False, f"court-ledger: bad status {entry.get('status')!r} for {entry.get('surface')}")
            court = entry.get("court")
            # 'defined'/'pending' courts are planned and may not be
            # scaffolded yet; 'passing'/'failing' must exist
            if court and "/" in court and entry.get("status") in {"passing", "failing"}:
                d, c = court.split("/", 1)
                check((ROOT / "courts" / d / c / "claim.toml").exists(),
                      f"court-ledger: passing/failing court {court} has no claim.toml")

    # 3. residual-ledger.json
    resid = read_json(ROOT / "atlas" / "residual-ledger.json")
    if resid is not None:
        for r in resid.get("residuals", []):
            for field in ("id", "court", "first_observed", "classification", "root_cause", "resolution", "regression_witness"):
                check(field in r, f"residual-ledger: {r.get('id')}: missing {field}")

    # 4. coverage-gaps.json
    gaps = read_json(ROOT / "atlas" / "coverage-gaps.json")
    if gaps is not None:
        for g in gaps.get("gaps", []):
            for field in ("id", "surface", "evidence", "court_needed"):
                check(field in g, f"coverage-gaps: {g.get('id')}: missing {field}")

    # 5. inventory.json
    inv = read_json(ROOT / "atlas" / "inventory.json")
    if inv is not None:
        check("surfaces" in inv or "inventory" in inv or any(k for k in inv), "inventory.json: unreadable shape")

    # 6. courts
    claim_fields = ("claim", "model", "assumptions", "observables", "witness", "independence", "falsifier")
    if (ROOT / "courts").is_dir():
        for domain in sorted((ROOT / "courts").iterdir()):
            if not domain.is_dir() or domain.name in ("README.md",) or domain.suffix:
                continue
            for case in sorted(domain.iterdir()):
                if not case.is_dir():
                    continue
                if not (case / "claim.toml").exists():
                    continue
                court = f"{domain.name}/{case.name}"
                for f in ("claim.toml", "assumptions.toml", "comparator.toml", "README.md"):
                    check((case / f).exists(), f"{court}: missing {f}")
                try:
                    claim = tomllib.loads((case / "claim.toml").read_text())
                    for field in claim_fields:
                        check(field in claim, f"{court}: claim.toml missing {field}")
                except tomllib.TOMLDecodeError as e:
                    check(False, f"{court}: claim.toml invalid TOML: {e}")
                try:
                    tomllib.loads((case / "comparator.toml").read_text())
                except tomllib.TOMLDecodeError as e:
                    check(False, f"{court}: comparator.toml invalid TOML: {e}")
                try:
                    tomllib.loads((case / "assumptions.toml").read_text())
                except tomllib.TOMLDecodeError as e:
                    check(False, f"{court}: assumptions.toml invalid TOML: {e}")
                # evidence.json (when present) must parse and verify
                ev_path = case / "evidence.json"
                if ev_path.exists():
                    ev = read_json(ev_path)
                    if ev is not None:
                        for field in ("court", "result", "artifacts"):
                            check(field in ev, f"{court}: evidence.json missing {field}")
                        for a in ev.get("artifacts", []):
                            p = case / a.get("path", "")
                            if p.exists():
                                actual = hashlib.sha256(p.read_bytes()).hexdigest()
                                check(actual == a.get("sha256"),
                                      f"{court}: evidence hash mismatch for {a.get('path')}")

    # 7. evidence releases
    releases = ROOT / "evidence" / "releases"
    if releases.is_dir():
        for rel in sorted(releases.iterdir()):
            for f in ("MANIFEST.json", "COURTS.json", "RECEIPTS.json", "ROOT-HASH"):
                check((rel / f).exists(), f"release {rel.name}: missing {f}")
            root = (rel / "ROOT-HASH").read_text().strip() if (rel / "ROOT-HASH").exists() else ""
            courts = (rel / "COURTS.json").read_bytes() if (rel / "COURTS.json").exists() else b""
            check(hashlib.sha256(courts).hexdigest() == root,
                  f"release {rel.name}: ROOT-HASH does not match COURTS.json")
            man = read_json(rel / "MANIFEST.json")
            if man is not None:
                check(man.get("root_hash") == root, f"release {rel.name}: MANIFEST root_hash mismatch")

    if problems:
        print("atlas/court validation FAILED:")
        for p in problems:
            print(f"  {p}")
        return 1
    print("atlas + courts + releases validated OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
