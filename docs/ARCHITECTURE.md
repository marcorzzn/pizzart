# Architettura: decisioni e numeri

Documento delle scelte strutturali, con le misure che le hanno determinate. Non è
il diario di bordo (quello è `DEVLOG.md`): qui c'è ciò che è costoso cambiare dopo.

---

## 1. Formato pixel interno: **RGBA8 oggi, con un percorso dichiarato verso f16**

### Stato attuale

Ogni layer è RGBA8 (`Layer::pixels: Vec<u8>`, 4 byte per pixel). Ogni pass di
effetto quantizza a 8 bit: la catena di 8 pass misura **5/255** di scarto rispetto
al riferimento ideale (tolleranza dichiarata nel test `catena_blur_sharpen_glow`).

### La decisione

**Si resta su RGBA8** per questa fase, e si introduce un punto di indirezione
(`PixelFormat`) *prima* di aggiungere altri effetti, non dopo.

Motivazione, in ordine di peso:

1. **La memoria è già al limite.** Misurato: 24 MP (6000×4000) con 5 effetti →
   picco **1,87 GB working set, 2,00 GB privata**, ~1 s per effetto. Un documento
   del genere con RGBA16F passerebbe a **~4 GB**. Su una macchina da 8 GB è la
   differenza tra usabile e non avviabile.
2. **Il readback raddoppierebbe.** Il trasferimento GPU→CPU è già il collo di
   bottiglia della fluidità (27 ms/frame in preview a metà lato). Passare a f16
   significa 16 MB per frame invece di 8, prima di qualsiasi ottimizzazione.
3. **La parità CPU/GPU è verificata in 8 bit.** Tutti i 55 test di `render`
   confrontano campioni u8 con tolleranze misurate. In f16 la tolleranza andrebbe
   ridisegnata — non è un dettaglio, è la validità del contratto.
4. **L'errore accumulato è accettabile per un editor di consumo**: 5/255 su una
   catena di 8 effetti non è visibile a occhio. **Non è accettabile per la stampa**
   o per l'export HDR — ed è esattamente il criterio che dovrebbe far scattare il
   cambio.

### Quando andrebbe rifatta (i trigger espliciti)

- Si vuole esportare in 16 bit per canale (TIFF/PNG a 16 bit) per la stampa.
- Si aggiungono più di ~6 pass in catena su un solo layer senza ricampionare.
- Si introduce una LUT colore: una LUT applicata in 8 bit quantizza la mappatura
  a 256 gradini e lo si vede sulle sfumature.

### Cosa cambierebbe, crate per crate

| Crate | Cambiamento | Costo |
|---|---|---|
| `core` | `Layer::pixels` da `Vec<u8>` a `Vec<half>`/`Vec<f32>`; `PlacedImage` uguale; blend e compositor riscritti sui nuovi tipi; **tutti** i 118 test vanno rivisti (confronti byte-per-byte non hanno più senso) | **L** |
| `draw` | la tessera dello stroke nasce in f16; `coverage` resta u8 (è una maschera di copertura, non colore) | M |
| `text` | idem: la maschera del glifo resta u8, il colore diventa f16 | S |
| `render` | formati wgpu `Rgba16Float`/`Rgba32Float`, shader riscritti sui nuovi tipi, CPU reference riscritto, **tutte le tolleranze rimisurate** | **XL** |
| `io` | export: PNG 16 bit, JPEG sempre 8 bit (conversione in uscita) | M |
| `project` | il blob dei pixel raddoppia; `byte_size` e codec restano validi (Zstd comprime anche f16, peggio) | S |
| `ui` | nessun cambiamento logico, ma la texture egui resta 8 bit: serve una conversione in presentazione | S |

**Totale: 3–4 settimane**, con il rischio concentrato in `render` (parità da
rimisurare) e in `core` (tutti i test da riscrivere).

### La mossa a costo quasi zero, da fare prima

Introdurre `pizzart_core::PixelFormat` con la sola variante `Rgba8`, e far
passare da lì i formati wgpu in `render`. Non cambia nulla oggi; il giorno in cui
si aggiunge `Rgba16Float`, il punto di verità esiste già e non va cercato in venti
file. **Stima: mezza giornata.** È l'unica parte di questa decisione che
consiglierei di eseguire subito.

---

## 2. Memoria: il modello a tessera intera ha un tetto noto

### Misura (release, AMD R5, sonda temporanea su 24 MP)

| Operazione | Tempo | Memoria |
|---|---|---|
| Documento 6000×4000, 1 layer | — | 96 MB di pixel |
| 5 effetti applicati (uno per layer) | ~1,0 s ciascuno | **picco 1,87 GB WS / 2,00 GB privata** |
| Stato finale: 6 layer | — | 576 MB di pixel residenti |

Crescita lineare: **+96 MB per layer** a 24 MP, più 3 texture temporanee (288 MB)
e il buffer di lettura durante ogni pass di effetto.

### Conseguenze

- Il modello "tessera intera + ping-pong a 3 texture" è **sostenibile fino a
  8–10 MP** con più layer; a 24 MP si arriva vicini al limite su una macchina
  da 8 GB.
- `apply_effect` **duplica** i pixel del layer (BUG-009 del report UI): è la voce
  più facile da tagliare, e vale 96 MB per effetto a 24 MP.
- La strada per andare oltre: **tile-based rendering** (comporre a blocchi di
  256×256 e non allocare mai il buffer intero). È un cambio di `render` e di
  `compositor`, non di `core`: i layer restano interi, la composizione procede a
  blocchi. Stima **1–2 settimane**.

---

## 3. Renderer della UI: glow e wgpu convivono, ed è il costo noto

Verificato con `cargo tree -p pizzart-ui`:

```
wgpu v23.0.1
└── pizzart-render
    └── pizzart-ui

glow v0.14.2
├── eframe
└── egui_glow
    └── pizzart-ui
```

Due stack grafici nello stesso processo: **wgpu** per il motore (compositing ed
effetti), **glow/OpenGL** per l'interfaccia. Il composito viene quindi scaricato
GPU→CPU e ricaricato come texture egui: è la ragione dei **27 ms/frame** misurati
in preview, e il motivo per cui il drag non raggiunge i 60 fps.

Il fix strutturale (condividere la texture) richiede `eframe` con backend wgpu e
`egui_wgpu::CallbackTrait`, quindi **la stessa versione di wgpu** nei due stack.
Va valutato in Fase 2 dopo aver misurato separatamente `render()` e `read_rgba()`:
se il costo è nella sincronizzazione (`Maintain::Wait`), il doppio buffer
asincrono è più economico del cambio di backend.

---

## 4. Altre decisioni già prese (riferimento)

- **Documento ad albero** (`Node::Layer | Node::Group`): i gruppi compongono in un
  buffer proprio e applicano i loro attributi al risultato. Niente *pass-through*.
- **Identificativi**: `LayerId` (contatore) in memoria, `StableId` a 16 byte
  (forma UUID v4) nel file. Nessuna dipendenza esterna nel core.
- **Salvataggio**: zip con `manifest.json` + asset Zstd livello 1. Maschere
  condivise deduplicate.
- **Testo rasterizzato**: un testo diventa una tessera, non un layer vivo. È la
  ragione per cui il formato di progetto non contiene font.
