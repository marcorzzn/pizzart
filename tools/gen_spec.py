#!/usr/bin/env python3
"""Genera le specifiche di pizzart a partire dai dati estratti dall'APK.

Questo script legge gli artefatti di analisi (che restano FUORI dal repository,
in `_estrazione/`) e produce documenti DERIVATI: inventari, tabelle, conteggi.

Cosa fa e cosa non fa:
  - estrae fatti (nomi di schermate, deeplink, catalogo effetti, terminologia UI);
  - non copia il codice decompilato, le risorse o gli asset dell'app;
  - non riproduce la tabella delle stringhe integrale: ne deriva un vocabolario.

Uso:
    python tools/gen_spec.py [--input CARTELLA] [--out CARTELLA]

Default: --input ../../_estrazione   --out ../docs/spec
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import textwrap
import xml.etree.ElementTree as ET
from collections import Counter, OrderedDict, defaultdict
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"

# ---------------------------------------------------------------- utilità

def cell(text: str) -> str:
    """Rende una stringa sicura dentro una cella di tabella markdown."""
    return str(text).replace("|", "\\|").replace("\n", " ").strip()


def short_class(fqcn: str) -> str:
    return fqcn.rsplit(".", 1)[-1]


def fmt_link(link: dict) -> str:
    s = link["scheme"]
    h = link["host"]
    p = link["path"].lstrip("/")
    text = f"{s}://{h}" if s and h else (f"{s}:" if s else h)
    if p:
        text += ("/" if not text.endswith(":") else "") + p
    return f"`{text}`"


def fmt_links(links: list[dict], maxn: int) -> str:
    """Elenca fino a `maxn` ingressi, poi riassume: le tabelle restano leggibili."""
    shown = [fmt_link(l) for l in links[:maxn]]
    rest = len(links) - len(shown)
    return ", ".join(shown) + (f" (+{rest})" if rest > 0 else "")


# ------------------------------------------------- 1. schermate (manifest)

# Aree funzionali derivate dal nome della classe. L'ordine conta: vince il primo.
# Assunzione di perimetro: pizzart è un EDITOR, non un social network.
# Le aree "fuori perimetro" sono elencate ma non implementate; per cambiarle
# basta spostare la voce qui sotto o toglierla da FUORI_PERIMETRO.
AREE = [
    (("picsart.ads", "mediation", "mraid", "mopub", "ironsource", "adcolony", "vungle"),
     "Pubblicità (fuori perimetro)"),
    (("subscription", "premium", "paywall", "billing", "purchase"), "Abbonamento (fuori perimetro)"),
    (("facebook", "snapchat", "facebook.ads", "google.ads", "gms.ads", "dropbox", "kakao",
      "telekom", "line", "twitter", "pinterest", "instagram", "auth.google", "firebase"),
     "SDK terzi (fuori perimetro)"),
    (("challenge", "discover", "feed", "comment", "message", "follow", "social",
      "activity.center", "explore", "collection", "tags", "users", "sharing"),
     "Social (fuori perimetro)"),
    (("shop", "store"), "Store (fuori perimetro)"),
    (("miniapp", "mini_app"), "Mini-app (fuori perimetro)"),
    (("analytics", "debug", "experiment", "telemetry", "country"), "Telemetria (fuori perimetro)"),
    (("clevertap", "applovin", "inmobi", "fyber", "unity3d", "amazon", "vk", "androidx.test",
      "gms.common"), "SDK terzi (fuori perimetro)"),
    (("deeplink",), "Deeplink / ingresso"),
    (("chooser", "gallery", "picker", "photonav", "ingest", "media"), "Libreria / importazione"),
    (("editor.video", "video", "blooper", "replay"), "Video"),
    (("collage",), "Collage"),
    (("addobjects", "lensflare", "sticker", "font", "typeface"), "Oggetti (testo, sticker, forme)"),
    (("effect", "filter", "texture"), "Effetti"),
    (("draw", "brush"), "Disegno"),
    (("camera", "capture", "scan"), "Fotocamera"),
    (("preference", "settings", "notifications"), "Impostazioni"),
    (("onboard", "intro", "splash", "welcome", "tutorial", "growth", "terms", "launch"),
     "Onboarding"),
    (("login", "auth", "account", "signup", "signin", "profile"), "Account"),
    (("template",), "Template"),
    (("crop", "resize", "transform"), "Ritaglio / formato"),
    (("export", "share", "save", "print"), "Export / condivisione"),
    (("createflow", "create.selection", "create"), "Creazione guidata"),
    (("search",), "Ricerca"),
    (("projects", "userprojects"), "Progetti"),
    (("detection", "info"), "Informazioni / dialoghi"),
    (("dialog", "popup", "sheet"), "Dialoghi"),
    (("ai", "magic", "gen", "text_2_image"), "AI generativa"),
    (("webview", "embedded", "browser", "web"), "Web embedded"),
    (("audio", "music", "sound"), "Audio"),
    (("home", "main", "pager", "studio"), "Editor (superficie principale)"),
    (("photo", "picsinphoto"), "Editor (superficie principale)"),
]


def _match(low: str, key: str) -> bool:
    """Match a confini di parola: `social` non deve colpire `socialin`."""
    return re.search(r"(?<![a-z])" + re.escape(key) + r"(?![a-z])", low) is not None


def area_of(fqcn: str) -> str:
    low = fqcn.lower()
    for keys, area in AREE:
        if any(_match(low, k) for k in keys):
            return area
    return "Altro"

# Aree che pizzart non implementa: sono la ragione del progetto.
FUORI_PERIMETRO = {area for _, area in AREE if "fuori perimetro" in area}


def parse_manifest(path: Path) -> dict:
    root = ET.parse(path).getroot()
    ns = ANDROID
    out = {
        "package": root.get("package", ""),
        "version_name": root.get(f"{ns}versionName"),
        "version_code": root.get(f"{ns}versionCode"),
        "min_sdk": None,
        "target_sdk": None,
        "activities": [],
        "services": [],
        "receivers": [],
        "providers": [],
        "permissions": [],
        "features": [],
    }

    app = root.find("application")
    if app is not None:
        tag_key = {
            "activity": "activities",
            "activity-alias": "activities",
            "service": "services",
            "receiver": "receivers",
            "provider": "providers",
        }
        for tag, key in tag_key.items():
            for el in app.findall(tag):
                name = el.get(f"{ns}name", "")
                if not name:
                    continue
                entry = {
                    "class": name,
                    "short": short_class(name),
                    "exported": el.get(f"{ns}exported", ""),
                    "area": area_of(name),
                    "actions": [],
                    "deeplinks": [],
                    "authorities": el.get(f"{ns}authorities", ""),
                }
                for filt in el.findall("intent-filter"):
                    for act in filt.findall("action"):
                        a = act.get(f"{ns}name", "")
                        if a and a not in entry["actions"]:
                            entry["actions"].append(a)
                    for data in filt.findall("data"):
                        link = {
                            "scheme": data.get(f"{ns}scheme", ""),
                            "host": data.get(f"{ns}host", ""),
                            "path": data.get(f"{ns}pathPrefix") or data.get(f"{ns}path", ""),
                        }
                        if (link["scheme"] or link["host"]) and link not in entry["deeplinks"]:
                            entry["deeplinks"].append(link)
                out[key].append(entry)

    for el in root.findall("uses-permission") + root.findall("uses-permission-sdk-23"):
        p = el.get(f"{ns}name", "")
        if p:
            out["permissions"].append(p)
    for el in root.findall("uses-feature"):
        f = el.get(f"{ns}name", "")
        if f:
            out["features"].append(f)
    sdk = root.find("uses-sdk")
    if sdk is not None:
        out["min_sdk"] = sdk.get(f"{ns}minSdkVersion")
        out["target_sdk"] = sdk.get(f"{ns}targetSdkVersion")
    return out


def write_schermate(man: dict, out_dir: Path) -> dict:
    acts = man["activities"]
    per_area = Counter(a["area"] for a in acts)
    deeplinks = [a for a in acts if a["deeplinks"]]
    aree_utili = {k: v for k, v in per_area.items() if k not in FUORI_PERIMETRO}
    totals = {
        "attivita": len(acts),
        "deeplink": len(deeplinks),
        "servizi": len(man["services"]),
        "ricevitori": len(man["receivers"]),
        "provider": len(man["providers"]),
        "permessi": len(man["permissions"]),
        "feature_hw": len(man["features"]),
        "aree_utili": len(aree_utili),
    }

    L: list[str] = []
    A = L.append
    A("# Schermate di Picsart — inventario derivato")
    A("")
    A("> Documento **generato** da `tools/gen_spec.py` leggendo l'`AndroidManifest.xml`")
    A(f"> decodificato di `{man['package']}` {man['version_name'] or ''} (`versionCode` {man['version_code'] or '?'}).".rstrip())
    A("> Non contiene codice né risorse dell'app: solo fatti strutturali (nomi di componenti,")
    A("> azioni, schemi di deeplink). Serve come capitolato per le schermate di pizzart.")
    A("")
    A("## Numeri")
    A("")
    A("| Componente | Conteggio |")
    A("|---|---:|")
    A(f"| Activity | {totals['attivita']} |")
    A(f"| di cui con deeplink | {totals['deeplink']} |")
    A(f"| Service | {totals['servizi']} |")
    A(f"| Receiver | {totals['ricevitori']} |")
    A(f"| Content provider | {totals['provider']} |")
    A(f"| Permessi dichiarati | {totals['permessi']} |")
    A(f"| Feature hardware | {totals['feature_hw']} |")
    A("")
    A("## Aree funzionali")
    A("")
    A("Le aree sono derivate dal nome del package della classe (regola deterministica,")
    A("definita in `AREE` dentro il generatore).")
    A("")
    A("| Area | Activity | Da implementare in pizzart |")
    A("|---|---:|:---:|")
    for area, n in per_area.most_common():
        A(f"| {cell(area)} | {n} | {'no' if area in FUORI_PERIMETRO else 'sì'} |")
    A("")
    A("**Assunzione di perimetro:** pizzart è un editor, non un social network.")
    A("Pubblicità, abbonamento, social, store e mini-app restano fuori: sono esattamente")
    A("il bloatware che il progetto vuole eliminare. L'assunzione è una riga di codice")
    A("(`FUORI_PERIMETRO` nel generatore) e si può ribaltare se serve.")
    A("")
    A(f"Activity utili al perimetro: **{sum(aree_utili.values())}** su {totals['attivita']}.")
    A("")

    A("## Deeplink (ingressi esterni)")
    A("")
    A("| Ingresso | Activity | Area |")
    A("|---|---|---|")
    for a in sorted(deeplinks, key=lambda x: (x["area"], x["short"])):
        A(f"| {cell(fmt_links(a['deeplinks'], 3))} | {cell(a['short'])} | {cell(a['area'])} |")
    A("")
    routers = [a for a in acts if len(a["deeplinks"]) > 15]
    for r in routers:
        paths = sorted({l["path"] for l in r["deeplinks"] if l["path"]})
        schemes = sorted({l["scheme"] for l in r["deeplinks"] if l["scheme"]})
        A(f"### Mappa delle rotte (`{r['short']}`)")
        A("")
        A(f"È il router universale dell'app: {len(r['deeplinks'])} combinazioni dichiarate,")
        A(f"schemi {cell(', '.join('`' + s + '`' for s in schemes))}, {len(paths)} percorsi distinti.")
        A("Questa è la tassonomia di navigazione da cui derivare le rotte di pizzart.")
        A("")
        A("| # | percorso |")
        A("|---:|---|")
        for i, p in enumerate(paths, 1):
            A(f"| {i} | `{cell(p)}` |")
        A("")

    A("## Activity per area (solo aree nel perimetro)")
    A("")
    grouped: dict[str, list] = defaultdict(list)
    for a in acts:
        if a["area"] not in FUORI_PERIMETRO:
            grouped[a["area"]].append(a)
    for area in [x for x in per_area if x not in FUORI_PERIMETRO]:
        items = sorted(grouped.get(area, []), key=lambda x: x["short"])
        if not items:
            continue
        A(f"### {area} ({len(items)})")
        A("")
        A("| Schermata | Classe | Deeplink |")
        A("|---|---|---|")
        for a in items:
            A(f"| {cell(a['short'])} | `{cell(a['class'].rsplit('.', 1)[0])}` | {cell(fmt_links(a['deeplinks'], 2)) or '—'} |")
        A("")

    A("## Componenti dichiarati ma non replicati")
    A("")
    A("### Service")
    A("")
    for s in sorted(man["services"], key=lambda x: x["short"]):
        A(f"- `{s['short']}` — {s['area']}")
    A("")
    A("### Receiver")
    A("")
    for s in sorted(man["receivers"], key=lambda x: x["short"]):
        A(f"- `{s['short']}` — {s['area']}")
    A("")
    A("### Content provider")
    A("")
    for s in sorted(man["providers"], key=lambda x: x["short"]):
        A(f"- `{s['short']}` — {x_authorities(s)}")
    A("")
    A("## Permessi: confronto con il target di pizzart")
    A("")
    A("I permessi marcati *mai* sarebbero una regressione di privacy se li chiedessimo.")
    A("")
    mai = {"READ_CONTACTS", "AD_ID", "ACCESS_ADSERVICES_ATTRIBUTION", "ACCESS_ADSERVICES_AD_ID",
           "ACCESS_ADSERVICES_TOPICS", "ACCESS_ADSERVICES_CUSTOM_AUDIENCE", "BILLING",
           "CHECK_LICENSE", "DETECT_SCREEN_CAPTURE", "ACTIVITY_RECOGNITION"}
    A("| Permesso | pizzart |")
    A("|---|---|")
    for p in man["permissions"]:
        verdict = "mai" if any(m in p for m in mai) else "solo se serve"
        A(f"| `{p}` | {verdict} |")
    A("")
    A("## Come si usa questo documento")
    A("")
    A(textwrap.dedent("""\
    - Una riga della tabella = una schermata che un utente Picsart si aspetta di trovare.
    - Le aree diventano i moduli della UI (pannello sinistro, canvas, pannello destro).
    - I deeplink diventano i comandi del command palette (stessa funzione, altra porta).
    - La mappa delle rotte è la tassonomia di navigazione: da lì nascono i comandi.
    - Le voci *fuori perimetro* non si implementano: sono il vantaggio competitivo.
    """).strip())
    A("")

    (out_dir / "schermate.md").write_text("\n".join(L) + "\n", encoding="utf-8")
    (out_dir / "schermate.json").write_text(
        json.dumps({"package": man["package"], "activities": acts}, ensure_ascii=False, indent=2),
        encoding="utf-8")
    return totals


def x_authorities(entry: dict) -> str:
    a = entry.get("authorities") or "—"
    return f"`{a}`" if a != "—" else "—"


# ----------------------------------------------------- 2. effetti (catalogo)

# Famiglie derivate dal nome tecnico dell'effetto. Ordine: vince il primo match.
# È una classificazione euristica, utile a pianificare il modulo effetti:
# non pretende di indovinare la formula, solo di raggruppare per lavoro simile.
FAMIGLIE = [
    (("param_", "enum_"), "Parametro"),
    (("blur", "bokeh", "focus", "tiltshift", "lensblur"), "Sfocatura / profondità"),
    (("sharp", "crisp", "clarity", "detail"), "Nitidezza / dettaglio"),
    (("lomo", "holga", "cinerama", "cross_process", "crossprocess", "fattal", "orton",
      "bleach", "drama", "neoncola", "warmamber", "pastelperfect", "vintage", "retro",
      "film", "analog", "fade"), "Pellicola / analogico"),
    (("bw", "mono", "gray", "sepia", "tint"), "Bianco e nero / tinta"),
    (("hue", "sat", "color", "vibrance", "vibrant", "balance", "temperature", "white",
      "brightness", "selectiveadjust"), "Colore"),
    (("light", "glow", "flare", "sun", "shadow", "highlight", "exposure", "hdr"), "Luce"),
    (("curve", "level", "contrast", "gamma", "tone", "invert", "negative"), "Curve / toni"),
    (("noise", "grain", "dust", "scratch"), "Grana / texture"),
    (("distort", "warp", "twirl", "bulge", "mirror", "kaleido", "shear", "wave",
      "swirl", "motion", "water", "liquify"), "Distorsione"),
    (("stencil", "sketch", "pencil", "draw", "paint", "oil", "cartoon", "caricature",
      "commic", "comic", "art"), "Stilizzazione artistica"),
    (("pixelize", "posterize", "emboss", "contour", "polygon", "convolution", "mosaic"),
     "Effetti di pixel"),
    (("vignette", "frame", "border"), "Vignettatura / cornice"),
    (("face", "skin", "eye", "teeth", "hair", "brow", "reshape", "body", "blemish"),
     "Ritocco di persona"),
    (("pattern", "texture", "paper"), "Texture / pattern"),
]


def famiglia_di(effect_id: str) -> str:
    low = effect_id.lower()
    if re.fullmatch(r"(effect_)?(effect)?\d+", low) or low == "gen_none":
        return "Non identificato (numerato)"
    for keys, fam in FAMIGLIE:
        if any(k in low for k in keys):
            return fam
    return "Altro"


def parse_effects(path: Path) -> list[dict]:
    """Formato: `[[Categoria]]` poi `[effetto_o_parametro]` con righe `xx = Nome`."""
    entries: list[dict] = []
    cur = None
    categoria = ""
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        s = raw.strip()
        if not s:
            continue
        if s.startswith("[[") and s.endswith("]]"):
            categoria = s[2:-2]
            continue
        m = re.match(r"^\[([A-Za-z0-9_]+)\]$", s)
        if m:
            cur = {"id": m.group(1), "categoria": categoria, "nomi": {}}
            entries.append(cur)
            continue
        m = re.match(r"^([A-Za-z-]{2,5})\s*=\s*(.+)$", s)
        if m and cur is not None:
            cur["nomi"][m.group(1)] = m.group(2).strip()
    return entries


def write_effetti(entries: list[dict], out_dir: Path) -> dict:
    for e in entries:
        e["tipo"] = ("parametro" if e["id"].startswith("effect_param_")
                     else "enum" if e["id"].startswith("effect_enum_")
                     else "effetto")
        e["famiglia"] = famiglia_di(e["id"])

    effetti = [e for e in entries if e["tipo"] == "effetto"]
    parametri = [e for e in entries if e["tipo"] == "parametro"]
    enum = [e for e in entries if e["tipo"] == "enum"]
    lingue = sorted({lang for e in entries for lang in e["nomi"]})
    per_fam = Counter(e["famiglia"] for e in effetti)

    L: list[str] = []
    A = L.append
    A("# Catalogo effetti — specifica derivata")
    A("")
    A("> Generato da `tools/gen_spec.py` leggendo la tabella di localizzazione degli")
    A("> effetti inclusa nell'app. Il file originale resta fuori dal repository: qui c'è")
    A("> solo l'inventario derivato (identificativo tecnico, nome inglese, nome italiano,")
    A("> famiglia funzionale).")
    A("")
    A("## Cosa contiene e cosa non contiene")
    A("")
    A("| Contiene | Non contiene |")
    A("|---|---|")
    A(f"| {len(entries)} elementi: {len(effetti)} effetti, {len(parametri)} parametri, {len(enum)} enum | formule o codice degli effetti |")
    A(f"| nomi localizzati in {len(lingue)} lingue (incl. italiano) | i valori o i range dei parametri |")
    A("| l'identificativo tecnico (`effect_*`) | shader, LUT, texture o asset |")
    A("")
    A("**Conseguenza pratica:** il catalogo dice *quali* effetti esistono e come si chiamano,")
    A("non *come* sono fatti. Il comportamento (formula, intensità, range) va progettato da")
    A("zero nel modulo effetti di pizzart, e testato su casi noti (un Lomo non è definibile")
    A("per contratto: è una scelta di prodotto nostra).")
    A("")
    A("## Ripartizione per famiglia")
    A("")
    A("| Famiglia | Effetti |")
    A("|---|---:|")
    for fam, n in per_fam.most_common():
        A(f"| {cell(fam)} | {n} |")
    A(f"| **Totale** | **{len(effetti)}** |")
    A("")

    A("## Effetti")
    A("")
    A("La colonna *implementazione* parte vuota: è lo stato di avanzamento del modulo effetti.")
    A("")
    A("| # | id tecnico | EN | IT | Famiglia | implementato |")
    A("|---:|---|---|---|---|:---:|")
    for i, e in enumerate(sorted(effetti, key=lambda x: (x["famiglia"], x["id"])), 1):
        A(f"| {i} | `{e['id']}` | {cell(e['nomi'].get('en', '—'))} | {cell(e['nomi'].get('it', '—'))} "
          f"| {cell(e['famiglia'])} | ☐ |")
    A("")
    A("## Parametri riutilizzabili")
    A("")
    A("I parametri sono il vocabolario dei controlli che l'utente si aspetta di trovare")
    A("nel pannello di un effetto (slider, selettore): vanno mappati 1:1 nel modulo effetti.")
    A("")
    A("| id tecnico | EN | IT |")
    A("|---|---|---|")
    for e in sorted(parametri, key=lambda x: x["id"]):
        A(f"| `{e['id']}` | {cell(e['nomi'].get('en', '—'))} | {cell(e['nomi'].get('it', '—'))} |")
    A("")
    if enum:
        A("## Enum (scelte discrete)")
        A("")
        A("| id tecnico | EN | IT |")
        A("|---|---|---|")
        for e in sorted(enum, key=lambda x: x["id"]):
            A(f"| `{e['id']}` | {cell(e['nomi'].get('en', '—'))} | {cell(e['nomi'].get('it', '—'))} |")
        A("")
    A("## Lingue disponibili nel catalogo")
    A("")
    A(", ".join(f"`{l}`" for l in lingue))
    A("")

    (out_dir / "effetti.md").write_text("\n".join(L) + "\n", encoding="utf-8")
    (out_dir / "effetti.json").write_text(
        json.dumps(entries, ensure_ascii=False, indent=2), encoding="utf-8")
    return {"totale": len(entries), "effetti": len(effetti),
            "parametri": len(parametri), "enum": len(enum), "lingue": len(lingue)}


# ------------------------------------------------------- 3. glossario (UI)

# Termini di categoria che un utente Picsart riconosce subito.
# L'etichetta italiana è una NOSTRA scelta di prodotto: l'app analizzata non
# include la locale italiana (le traduzioni stanno nei language split).
IT_TERMS = {
    # superfici e struttura
    "add_layer": "Aggiungi livello", "layers": "Livelli", "layer": "Livello",
    "opacity": "Opacità", "blend": "Fusione", "mask": "Maschera",
    "background": "Sfondo", "canvas": "Tela", "undo": "Annulla", "redo": "Ripeti",
    # strumenti
    "crop": "Ritaglia", "rotate": "Ruota", "resize": "Ridimensiona",
    "flip": "Rifletti", "brush": "Pennello", "eraser": "Gomma",
    "draw": "Disegna", "text": "Testo", "sticker": "Sticker", "shape": "Forma",
    "clone": "Clona", "heal": "Correggi", "selection": "Selezione",
    "lasso": "Lazo", "wand": "Bacchetta magica", "eyedropper": "Contagocce",
    "color": "Colore", "gradient": "Sfumatura", "blur": "Sfocatura",
    "sharpen": "Nitidezza", "filter": "Filtro", "effect": "Effetto",
    "effects": "Effetti", "adjust": "Regola", "saturation": "Saturazione",
    "contrast": "Contrasto", "brightness": "Luminosità", "exposure": "Esposizione",
    "temperature": "Temperatura", "vignette": "Vignettatura", "grain": "Grana",
    "shadow": "Ombra", "highlight": "Luce", "border": "Bordo",
    "frame": "Cornice", "outline": "Contorno", "stroke": "Tratto",
    "thickness": "Spessore", "pressure": "Pressione", "size": "Dimensione",
    "spacing": "Spaziatura", "hardness": "Durezza", "flow": "Flusso",
    # azioni
    "save": "Salva", "export": "Esporta", "share": "Condividi",
    "edit": "Modifica", "delete": "Elimina", "duplicate": "Duplica",
    "merge": "Unisci", "flatten": "Appiattisci", "reset": "Reimposta",
    "apply": "Applica", "cancel": "Annulla", "done": "Fatto", "close": "Chiudi",
    "select": "Seleziona", "select_all": "Seleziona tutto", "deselect": "Deseleziona",
    "more": "Altro", "settings": "Impostazioni", "preview": "Anteprima",
    # formati
    "png": "PNG", "jpg": "JPG", "webp": "WebP", "transparent": "Trasparente",
    "quality": "Qualità", "resolution": "Risoluzione", "format": "Formato",
}

FAMIGLIE_GLOSSARIO = [
    ("superficie", ("layer", "opacity", "blend", "mask", "background", "canvas", "undo", "redo")),
    ("strumenti", ("crop", "rotate", "resize", "flip", "brush", "eraser", "draw", "text",
                   "sticker", "shape", "clone", "heal", "selection", "lasso", "wand", "eyedropper")),
    ("colore", ("color", "gradient", "saturation", "contrast", "brightness", "exposure",
                "temperature", "filter", "effect", "adjust", "vignette", "grain", "blur", "sharpen")),
    ("stile", ("shadow", "highlight", "border", "frame", "outline", "stroke", "thickness",
               "pressure", "size", "spacing", "hardness", "flow")),
    ("azioni", ("save", "export", "share", "edit", "delete", "duplicate", "merge", "flatten",
                "reset", "apply", "cancel", "done", "close", "select", "deselect", "more",
                "settings", "preview")),
    ("formati", ("png", "jpg", "webp", "transparent", "quality", "resolution", "format")),
]


def parse_strings(path: Path) -> dict[str, str]:
    sx = path.read_text(encoding="utf-8", errors="replace")
    return dict(re.findall(r'<string name="([^"]+)"[^>]*>(.*?)</string>', sx, re.S))


def write_glossario(strings: dict[str, str], out_dir: Path) -> dict:
    kw = ("crop", "layer", "brush", "draw", "effect", "filter", "text", "sticker", "shape",
          "mask", "clone", "heal", "blur", "sharpen", "rotate", "resize", "opacity", "blend",
          "background", "color", "gradient", "shadow", "border", "frame", "outline", "stroke",
          "save", "export", "share", "delete", "duplicate", "merge", "flatten", "reset",
          "apply", "cancel", "done", "selection", "undo", "redo", "adjust", "saturation",
          "contrast", "brightness", "exposure", "temperature", "vignette", "grain", "quality",
          "resolution", "format", "preview", "settings", "eraser", "lasso", "wand")
    terms: dict[str, str] = {}
    for name, text in strings.items():
        low = name.lower()
        if not any(k in low for k in kw):
            continue
        if len(text) > 40 or re.search(r"%[0-9$sdf]", text) or "<" in text:
            continue
        # la chiave è il primo segmento del nome che è termine noto
        # (es. `add_objects_remove_background` -> `background`)
        for seg in re.split(r"[_.]", low):
            if seg in IT_TERMS:
                terms.setdefault(seg, text)
                break

    L: list[str] = []
    A = L.append
    A("# Glossario UI — specifica derivata")
    A("")
    A("> Generato da `tools/gen_spec.py` leggendo la tabella delle stringhe dell'app.")
    A("> Non è una copia della tabella: è il **vocabolario minimo** che serve perché un")
    A("> utente Picsart riconosca pizzart senza imparare una nuova interfaccia.")
    A("")
    A("## Perché questo documento")
    A("")
    A("La familiarità non sta nell'imitare i pixel: sta nei **nomi degli strumenti**.")
    A("Chi usa Picsart cerca «Ritaglia», non «Transform crop region». pizzart usa le")
    A("etichette della categoria, che sono parole comuni del dominio.")
    A("")
    A("## Nota sulle traduzioni")
    A("")
    strings_count = len(strings)
    A(f"L'app analizzata include **{strings_count}** stringhe nella sola locale di default:")
    A("in questa build **non esiste `values-it`** (verificato: 39 cartelle `values*`, nessuna")
    A("italiana), perché le traduzioni viaggiano nei language split dell'app bundle.")
    A("")
    A("Quindi: la colonna *EN osservato* è un fatto estratto dai dati, la colonna")
    A("**IT è una scelta di prodotto di pizzart** e come tale va revisionata.")
    A("")
    A("## Terminologia")
    A("")
    for fam, keys in FAMIGLIE_GLOSSARIO:
        rows = [(k, IT_TERMS[k], terms.get(k, "")) for k in keys]
        if not rows:
            continue
        A(f"### {fam.capitalize()}")
        A("")
        A("| chiave | etichetta IT (pizzart) | EN osservato nell'app |")
        A("|---|---|---|")
        for k, it, en in rows:
            A(f"| `{k}` | {cell(it)} | {cell(en) or '—'} |")
        A("")
    A("## Regole di naming")
    A("")
    A("1. Un concetto, una parola: se il pannello dice *Livello*, il comando dice *livello*.")
    A("2. Si usa il termine che l'utente conosce già, anche quando è meno preciso.")
    A("3. Niente marchi né nomi di effetto proprietari: `Lomo` diventa un preset nostro.")
    A("4. Il testo è nostro: nessuna stringa dell'app va copiata, solo il vocabolario.")
    A("")

    mancanti = sorted(k for k in IT_TERMS if k not in terms)
    if mancanti:
        A("## Termini che l'app non espone negli asset di default")
        A("")
        A("Questi concetti non compaiono nella tabella delle stringhe di questa build:")
        A("o vivono nei language split, o l'app non li nomina affatto nell'interfaccia.")
        A("Sono i candidati naturali ai differenziatori di pizzart (controlli più espliciti):")
        A("")
        A(", ".join(f"`{k}` ({IT_TERMS[k]})" for k in mancanti))
        A("")

    (out_dir / "glossario.md").write_text("\n".join(L) + "\n", encoding="utf-8")
    return {"stringhe": strings_count, "termini_it": len(IT_TERMS),
            "abbinati": sum(1 for k in IT_TERMS if k in terms)}


# ------------------------------------------------------------------- main

def main() -> int:
    root = Path(__file__).resolve().parent.parent          # .../pizzart
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", default=str(root.parent / "_estrazione"),
                    help="cartella con AndroidManifest.xml, localEffectsLib.txt, strings.xml")
    ap.add_argument("--out", default=str(root / "docs" / "spec"))
    args = ap.parse_args()

    src = Path(args.input)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    need = ["AndroidManifest.xml", "localEffectsLib.txt", "strings.xml"]
    missing = [n for n in need if not (src / n).exists()]
    if missing:
        print(f"ERRORE: mancano in {src}: {', '.join(missing)}", file=sys.stderr)
        print("Estraili prima con tools/estrai_input.py (o passali con --input).", file=sys.stderr)
        return 2

    man = parse_manifest(src / "AndroidManifest.xml")

    # Il manifest decodificato non porta più versionName/versionCode: li ha apktool.yml.
    meta = src / "apktool.yml"
    if meta.exists():
        y = meta.read_text(encoding="utf-8", errors="replace")
        m1 = re.search(r"versionName:\s*'?\"?([^'\"\n]+)", y)
        m2 = re.search(r"versionCode:\s*'?\"?(\d+)", y)
        if m1:
            man["version_name"] = m1.group(1).strip()
        if m2:
            man["version_code"] = m2.group(1)
    t1 = write_schermate(man, out)
    t2 = write_effetti(parse_effects(src / "localEffectsLib.txt"), out)
    t3 = write_glossario(parse_strings(src / "strings.xml"), out)

    print(f"schermate.md  -> {t1}")
    print(f"effetti.md    -> {t2}")
    print(f"glossario.md  -> {t3}")
    print(f"scritti in {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
