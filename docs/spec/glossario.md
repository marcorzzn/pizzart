# Glossario UI — specifica derivata

> Generato da `tools/gen_spec.py` leggendo la tabella delle stringhe dell'app.
> Non è una copia della tabella: è il **vocabolario minimo** che serve perché un
> utente Picsart riconosca pizzart senza imparare una nuova interfaccia.

## Perché questo documento

La familiarità non sta nell'imitare i pixel: sta nei **nomi degli strumenti**.
Chi usa Picsart cerca «Ritaglia», non «Transform crop region». pizzart usa le
etichette della categoria, che sono parole comuni del dominio.

## Nota sulle traduzioni

L'app analizzata include **3247** stringhe nella sola locale di default:
in questa build **non esiste `values-it`** (verificato: 39 cartelle `values*`, nessuna
italiana), perché le traduzioni viaggiano nei language split dell'app bundle.

Quindi: la colonna *EN osservato* è un fatto estratto dai dati, la colonna
**IT è una scelta di prodotto di pizzart** e come tale va revisionata.

## Terminologia

### Superficie

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `layer` | Livello | Add Layer |
| `opacity` | Opacità | Opacity |
| `blend` | Fusione | Blend colors in just one swipe! |
| `mask` | Maschera | Mask |
| `background` | Sfondo | No background to remove |
| `canvas` | Tela | — |
| `undo` | Annulla | Layers merged. Undo to edit. |
| `redo` | Ripeti | Redo |

### Strumenti

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `crop` | Ritaglia | Custom Crop Ratio |
| `rotate` | Ruota | — |
| `resize` | Ridimensiona | Resize Image |
| `flip` | Rifletti | Flip/Rotate has moved to Crop |
| `brush` | Pennello | Brush |
| `eraser` | Gomma | Eraser mode |
| `draw` | Disegna | Draw on Photo |
| `text` | Testo | Box |
| `sticker` | Sticker | Apply sticker |
| `shape` | Forma | Add Shape |
| `clone` | Clona | Clone |
| `heal` | Correggi | Heal |
| `selection` | Selezione | Auto |
| `lasso` | Lazo | Lasso |
| `wand` | Bacchetta magica | — |
| `eyedropper` | Contagocce | — |

### Colore

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `color` | Colore | Bg color |
| `gradient` | Sfumatura | — |
| `saturation` | Saturazione | Saturation |
| `contrast` | Contrasto | Contrast |
| `brightness` | Luminosità | Brightness |
| `exposure` | Esposizione | — |
| `temperature` | Temperatura | — |
| `filter` | Filtro | Glitch |
| `effect` | Effetto | Auto |
| `adjust` | Regola | Tap to adjust the result |
| `vignette` | Vignettatura | — |
| `grain` | Grana | — |
| `blur` | Sfocatura | Reducing blur |
| `sharpen` | Nitidezza | — |

### Stile

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `shadow` | Ombra | Angle |
| `highlight` | Luce | — |
| `border` | Bordo | BorderFragment |
| `frame` | Cornice | Looking for Frames? |
| `outline` | Contorno | Outline object to cut |
| `stroke` | Tratto | — |
| `thickness` | Spessore | — |
| `pressure` | Pressione | — |
| `size` | Dimensione | Size and Background |
| `spacing` | Spaziatura | — |
| `hardness` | Durezza | — |
| `flow` | Flusso | Color Backgrounds |

### Azioni

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `save` | Salva | Discover and Save |
| `export` | Esporta | BlooperExportFragment |
| `share` | Condividi | Share via email |
| `edit` | Modifica | Edit background |
| `delete` | Elimina | delete |
| `duplicate` | Duplica | Duplicate and open |
| `merge` | Unisci | Merge all |
| `flatten` | Appiattisci | — |
| `reset` | Reimposta | RESET |
| `apply` | Applica | Tap here to auto apply |
| `cancel` | Annulla | Not you? |
| `done` | Fatto | Done |
| `close` | Chiudi | Open/Close Layers Panel |
| `select` | Seleziona | Select Folder |
| `deselect` | Deseleziona | — |
| `more` | Altro | Tap here for more colors |
| `settings` | Impostazioni | Settings |
| `preview` | Anteprima | BlooperPreviewFragment |

### Formati

| chiave | etichetta IT (pizzart) | EN osservato nell'app |
|---|---|---|
| `png` | PNG | — |
| `jpg` | JPG | Saved to your device as a JPG |
| `webp` | WebP | — |
| `transparent` | Trasparente | Saves with a transparent background |
| `quality` | Qualità | Enhance image quality with AI |
| `resolution` | Risoluzione | Resolution |
| `format` | Formato | Invalid number format |

## Regole di naming

1. Un concetto, una parola: se il pannello dice *Livello*, il comando dice *livello*.
2. Si usa il termine che l'utente conosce già, anche quando è meno preciso.
3. Niente marchi né nomi di effetto proprietari: `Lomo` diventa un preset nostro.
4. Il testo è nostro: nessuna stringa dell'app va copiata, solo il vocabolario.

## Termini che l'app non espone negli asset di default

Questi concetti non compaiono nella tabella delle stringhe di questa build:
o vivono nei language split, o l'app non li nomina affatto nell'interfaccia.
Sono i candidati naturali ai differenziatori di pizzart (controlli più espliciti):

`add_layer` (Aggiungi livello), `canvas` (Tela), `deselect` (Deseleziona), `exposure` (Esposizione), `eyedropper` (Contagocce), `flatten` (Appiattisci), `gradient` (Sfumatura), `grain` (Grana), `hardness` (Durezza), `highlight` (Luce), `png` (PNG), `pressure` (Pressione), `rotate` (Ruota), `select_all` (Seleziona tutto), `sharpen` (Nitidezza), `spacing` (Spaziatura), `stroke` (Tratto), `temperature` (Temperatura), `thickness` (Spessore), `vignette` (Vignettatura), `wand` (Bacchetta magica), `webp` (WebP)

