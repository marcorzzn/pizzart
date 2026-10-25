#!/usr/bin/env python3
"""Estrae da pizzart solo i tre input che servono a generare le specifiche.

Gli archivi dell'analisi restano dove sono: qui si leggono in sola lettura e si
scrivono solo tre file dentro una cartella di lavoro, che NON fa parte del
repository del progetto (`../_estrazione`, ignorata da git).

Uso:
    python tools/estrai_input.py [--zips CARTELLA] [--out CARTELLA]
"""

from __future__ import annotations

import argparse
import sys
import zipfile
from pathlib import Path

# (nome archivio, percorso interno) — si prova il primo che esiste.
WANTED = {
    "AndroidManifest.xml": [
        ("picsplit_results.zip", "01_apktool/AndroidManifest.xml"),
        ("picsart_output.zip", "apktool/AndroidManifest.xml"),
    ],
    "localEffectsLib.txt": [
        ("picsplit_results.zip", "01_apktool/assets/localEffectsLib.txt"),
        ("picsart_output.zip", "apktool/assets/localEffectsLib.txt"),
    ],
    "strings.xml": [
        ("picsplit_results.zip", "01_apktool/res/values/strings.xml"),
        ("picsart_output.zip", "apktool/res/values/strings.xml"),
    ],
    "apktool.yml": [
        ("picsplit_results.zip", "01_apktool/apktool.yml"),
        ("picsart_output.zip", "apktool/apktool.yml"),
    ],
}


def find(zips: Path, candidates: list[tuple[str, str]]) -> tuple[Path, str] | None:
    for archive, member in candidates:
        path = zips / archive
        if not path.exists():
            continue
        try:
            with zipfile.ZipFile(path) as z:
                if member in z.namelist():
                    return path, member
        except zipfile.BadZipFile:
            print(f"  ! {archive}: archivio non valido", file=sys.stderr)
    return None


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--zips", default=str(root.parent), help="cartella con gli archivi dell'analisi")
    ap.add_argument("--out", default=str(root.parent / "_estrazione"), help="cartella di lavoro (fuori dal repo)")
    args = ap.parse_args()

    zips, out = Path(args.zips), Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    failures = 0
    for target, candidates in WANTED.items():
        hit = find(zips, candidates)
        if hit is None:
            print(f"MANCANTE: {target} (nessuno degli archivi lo contiene)", file=sys.stderr)
            failures += 1
            continue
        archive, member = hit
        with zipfile.ZipFile(archive) as z:
            data = z.read(member)
        dest = out / target
        dest.write_bytes(data)
        print(f"estratto {member:48} da {archive.name:24} -> {dest} ({len(data):,} byte)")

    # marcatore: rende esplicito che questa cartella non va versionata
    (out / ".gitignore").write_text("*\n", encoding="utf-8")
    if failures:
        print(f"{failures} input mancanti: impossibile generare le specifiche.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
