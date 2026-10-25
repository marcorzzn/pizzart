#!/usr/bin/env python3
"""Guardia: nel repository non entrano dati estratti dall'app di riferimento.

Perche' esiste
--------------
`tools/gen_spec.py` e' stato modificato per scrivere il dettaglio **fuori** dal
repository. Questa e' la seconda protezione, e la piu' importante: intercetta
qualunque introduzione accidentale di materiale di terzi, non solo la
rigenerazione automatica. Un copia-incolla da `_estrazione/` in un commento, in
un test o in un documento passa di qui e viene fermato.

Cosa cerca
----------
1. `com.picsart.`  -> nomi di classi Android dell'app (dump del manifest).
2. `effect_param_` / `effect_enum_` fuori da `tools/gen_spec.py` -> gli
   identificativi tecnici del catalogo. In `gen_spec.py` sono ammessi perche'
   sono una regola di classificazione, dichiarata in `docs/spec/README.md`.
3. Tabelle di localizzazione -> dieci o piu' lingue con la stessa forma
   `xx: "..."` nello stesso file: e' il sintomo di un dump di traduzioni.

Uscita: codice 0 se il repository e' pulito, 1 elencando file e riga.

I pattern sono composti a runtime: uno script che contiene le stringhe che
vieta si autodenuncia, e la prima vittima sarebbe la sua stessa riga.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

RADICE = Path(__file__).resolve().parent.parent

# Pattern composti per non autodenunciarsi (vedi docstring).
PADRE = "com"
APP = "picsart"

PATTERN_CLASSI = re.compile(rf"{PADRE}\.{APP}\.")
PATTERN_ID = re.compile(r"effect_(?:param|enum)_")

# Alcune lingue coperte dal catalogo dell'app: se ne compaiono dieci nello
# stesso file, e' una tabella di traduzioni, non una nota.
LINGUE = (
    "en", "it", "de", "fr", "es", "pt", "ru", "ja", "ko", "zh", "ar",
    "bn", "fa", "hi", "id", "ms", "th", "tr", "uk", "vi", "nl",
)
PATTERN_LINGUA = re.compile(rf'^\s*"?(?P<lingua>{"|".join(LINGUE)})"?\s*:\s*"', re.M)

# File ammessi, con la ragione.
ESCLUSI = {
    "tools/gen_spec.py": "contiene le regole di classificazione, dichiarate in docs/spec/README.md",
    "docs/spec/README.md": "spiega la posizione clean-room e nomina l'app per farlo",
    "tools/check_spec_no_dump.py": "questo script: i pattern sono composti a runtime",
}
# Cartelle fuori dal repository per definizione.
ESCLUSI_PREFISSI = ("_estrazione/", "target/", ".git/")

# Estensioni leggibili come testo.
ESTENSIONI = {
    ".rs", ".toml", ".md", ".json", ".py", ".yml", ".yaml",
    ".sh", ".txt", ".lock", ".wgsl", ".gitignore",
}


def file_versionati() -> list[str]:
    """Solo cio' che git traccia: quello che finisce nel repository."""
    esito = subprocess.run(
        ["git", "ls-files"],
        cwd=RADICE,
        capture_output=True,
        text=True,
        check=True,
    )
    return [riga for riga in esito.stdout.splitlines() if riga.strip()]


def leggi(percorso: Path) -> str | None:
    try:
        return percorso.read_text(encoding="utf-8", errors="strict")
    except (UnicodeDecodeError, OSError):
        return None


def main() -> int:
    problemi: list[str] = []
    controllati = 0

    for relativo in file_versionati():
        if relativo in ESCLUSI:
            continue
        if relativo.startswith(ESCLUSI_PREFISSI):
            continue
        percorso = RADICE / relativo
        if percorso.suffix.lower() not in ESTENSIONI and percorso.name != ".gitignore":
            continue
        testo = leggi(percorso)
        if testo is None:
            continue
        controllati += 1

        for numero, riga in enumerate(testo.splitlines(), start=1):
            if PATTERN_CLASSI.search(riga):
                problemi.append(
                    f"{relativo}:{numero}: nome di classe Android dell'app ({PADRE}.{APP}.*)"
                )
            if PATTERN_ID.search(riga):
                problemi.append(
                    f"{relativo}:{numero}: identificativo del catalogo (effect_param_/effect_enum_)"
                )

        lingue = {m.group("lingua") for m in PATTERN_LINGUA.finditer(testo)}
        if len(lingue) >= 10:
            problemi.append(
                f"{relativo}: tabella di localizzazione sospetta "
                f"({len(lingue)} lingue: {', '.join(sorted(lingue))})"
            )

    print(f"File versionati controllati: {controllati}")
    if problemi:
        print(f"\nTROVATI {len(problemi)} problemi:\n")
        for p in problemi:
            print(f"  {p}")
        print(
            "\nNel repository entrano solo codice originale e specifiche aggregate.\n"
            "Il dettaglio estratto dall'app vive in _estrazione/ (fuori dal repo).\n"
            "Vedi docs/spec/README.md per la ragione."
        )
        return 1

    print("Nessun dump dell'APK: repository pulito.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
