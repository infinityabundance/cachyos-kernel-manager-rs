#!/usr/bin/env python3
"""ts2json.py — convert the frozen Qt .ts translation files to the compact
JSON catalogs the Iced UI embeds (ui/i18n layer, Phase 8).

Authority: oracle/upstream/lang/cachyos-kernel-manager_<locale>.ts
(revision 6b4a373e, hash-verified by oracle/UPSTREAM.lock). The catalog
aliases come from cachyoskm_locale.qrc (the alias is the BARE code, e.g.
`zh-CN` — gap-009: a QLocale name like `zh_CN` does NOT match the alias).

Output per locale (written to crates/cachyos-kernel-manager-ui/translations/):

    {
      "locale": "zh-CN",
      "qrc_alias": "zh-CN",
      "source_ts": "cachyos-kernel-manager_zh-CN.ts",
      "entries": [
        {"context": "MainWindow", "source": "...", "translation": "...",
         "unfinished": false, "locations": ["km-window.cpp:144"]},
        ...
      ]
    }

Qt resolution semantics preserved:
- a message whose <translation> is EMPTY or carries type="unfinished" is
  SKIPPED by QTranslator::translate (the source text is returned);
- <translation> type="vanished" is also not translated;
- duplicate (context, source) pairs: the FIRST matching <message> wins.

Usage: tools/ts2json.py [--check]
  --check  verify the checked-in JSON is up to date with the .ts files
           (CI gate: python3 tools/ts2json.py --check).
"""

import hashlib
import json
import os
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TS_DIR = os.path.join(ROOT, "oracle", "upstream", "lang")
QRC = os.path.join(ROOT, "oracle", "upstream", "cachyoskm_locale.qrc")
OUT_DIR = os.path.join(ROOT, "crates", "cachyos-kernel-manager-ui", "translations")


def qrc_aliases() -> dict:
    """locale.ts basename -> qrc alias (the <file alias=...> attribute)."""
    tree = ET.parse(QRC)
    aliases = {}
    for f in tree.iter("file"):
        alias = f.get("alias")
        src = f.text.strip() if f.text else ""
        # src = lang/cachyos-kernel-manager_<locale>.qm
        basename = os.path.basename(src)
        locale = basename.removeprefix("cachyos-kernel-manager_").removesuffix(".qm")
        aliases[locale] = alias
    return aliases


def parse_ts(path: str) -> list:
    """Parse one .ts -> entries (context, source, translation, unfinished)."""
    tree = ET.parse(path)
    entries = []
    for context in tree.iter("context"):
        name_el = context.find("name")
        ctx = name_el.text.strip() if name_el is not None and name_el.text else ""
        for msg in context.findall("message"):
            source_el = msg.find("source")
            if source_el is None or source_el.text is None:
                continue
            source = source_el.text
            trans_el = msg.find("translation")
            trans = trans_el.text if trans_el is not None and trans_el.text else ""
            trans_type = trans_el.get("type") if trans_el is not None else None
            locations = []
            for loc in msg.findall("location"):
                fn = loc.get("filename", "")
                line = loc.get("line", "")
                locations.append(f"{fn}:{line}" if line else fn)
            entries.append(
                {
                    "context": ctx,
                    "source": source,
                    "translation": trans,
                    "unfinished": trans_type in ("unfinished", "vanished"),
                    "locations": locations,
                }
            )
    return entries


def locale_source_sha256(path: str) -> str:
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def convert_all() -> dict:
    aliases = qrc_aliases()
    out_manifests = {}
    for locale, alias in sorted(aliases.items()):
        ts = os.path.join(TS_DIR, f"cachyos-kernel-manager_{locale}.ts")
        if not os.path.exists(ts):
            raise SystemExit(f"missing .ts for qrc alias {alias}: {ts}")
        entries = parse_ts(ts)
        doc = {
            "locale": locale,
            "qrc_alias": alias,
            "source_ts": f"cachyos-kernel-manager_{locale}.ts",
            "source_sha256": locale_source_sha256(ts),
            "entries": entries,
        }
        out_manifests[alias] = doc
    return out_manifests


def main() -> int:
    docs = convert_all()
    if "--check" in sys.argv:
        # verify the checked-in JSON matches a fresh conversion
        for alias, doc in docs.items():
            path = os.path.join(OUT_DIR, f"{alias}.json")
            with open(path, encoding="utf-8") as f:
                checked = json.load(f)
            if checked != doc:
                print(f"ts2json: {alias}.json is STALE (re-run tools/ts2json.py)")
                return 1
        print(f"ts2json: {len(docs)} catalogs up to date")
        return 0

    os.makedirs(OUT_DIR, exist_ok=True)
    for alias, doc in docs.items():
        path = os.path.join(OUT_DIR, f"{alias}.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump(doc, f, ensure_ascii=False, indent=1)
            f.write("\n")
    print(f"ts2json: wrote {len(docs)} catalogs to {OUT_DIR}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
