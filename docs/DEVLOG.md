# Diario di bordo

> **Diario di bordo del progetto.** Non è il README: qui c'è la storia delle
> decisioni, con i numeri misurati e gli errori trovati. Per l'essenziale vedi
> [README](../README.md); per le scelte strutturali [ARCHITECTURE](ARCHITECTURE.md).

MIT — vedi [LICENSE](LICENSE). Copyright (c) 2025 Marco Razzano.

Editor grafico professionale in Rust. Non è un port di Picsart: è un editor
pensato per essere più veloce e più estendibile, con le funzioni che servono e
senza quello che non serve.

## Posizione sul progetto originale

Pizzart è un editor **clean-room**: l'analisi dell'APK di Picsart (in
`_estrazione/`, fuori dal repository) è servita esclusivamente come
**capitolato funzionale** — quali schermate, quali effetti, quali strumenti
un utente si aspetta. Non è stato tradotto né riusato codice, non sono stati
copiati asset (icone, font, texture), non è stata replicata l'interfaccia
pixel per pixel. Le specifiche in `docs/spec/` sono **fatti aggregati**
(quanti effetti, quante famiglie), non dump di stringhe originali.
## Perché

Un utente Picsart deve poter aprire pizzart e **riconoscere** gli strumenti:
stessi nomi, stessa organizzazione, stesse schermate. Quello che cambia è ciò che
sta sotto (Rust, GPU, nessun runtime pesante) e ciò che non c'è: pubblicità,
paywall, tracciamento, sessanta SDK di terze parti.

## Build e test

Prerequisiti: Rust stable (`rustup install stable`), e su Windows i Visual Studio
Build Tools (workload "Desktop development with C++": serve a `zstd-sys` e a wgpu).

```bash
# Test (offline, veloce, nessuna GPU richiesta)
cargo test --offline -p pizzart-core -p pizzart-draw -p pizzart-text \
                     -p pizzart-io -p pizzart-project

# Test con GPU (parita' CPU/GPU, lento)
cargo test --offline -p pizzart-render -p pizzart-ui

# Avvio UI
cargo run --release -p pizzart-ui
```

**Nota su `Cargo.lock`**: e' committato deliberatamente. Pizzart e' un'applicazione,
non una libreria: la build deve essere riproducibile sulla macchina dell'utente, e
le versioni bloccate sono parte della garanzia. Se in futuro un crate verra'
pubblicato separatamente su crates.io come libreria, quel crate avra' il suo
`.gitignore`.

**Tempi di build**: la prima `cargo build --release` richiede 3-8 minuti (dipende
dalla CPU; con egui misurati ~18 minuti su questa macchina). Le successive sono in
cache: 10-30 secondi.

**CI**: il workflow in `.github/workflows/ci.yml` esegue i crate senza GPU su
Windows, Linux e macOS, piu' `fmt`, `clippy -D warnings` e la guardia
`tools/check_spec_no_dump.py` che impedisce ai dati estratti dall'app di rientrare
nel repository.
## Stato

| Componente | Stato |
|---|---|
| `crates/core` — documento, albero layer/gruppi, 12 blend, undo/redo, `Mask`, selezione | ✅ 95 + 19 test |
| `crates/draw` — pressione, tilt, spazzole procedurali, stabilizzazione, **dirty rect** | ✅ 22 test, `stroke_demo.png` |
| `crates/text` — registro font, layout, wrapping, rasterizzazione in tessera | ✅ 14 test, `text_demo.png` |
| `crates/render` — renderer wgpu, compositing ad albero con parità CPU | ✅ 55 test, `output.png` |
| `crates/io` — export PNG / JPEG / WebP, con o senza alpha | ✅ 13 test, `export_demo.*` |
| `crates/project` — salvataggio/caricamento: zip + manifest + Zstd | ✅ 14 test, `roundtrip` |
| crates/ui — UI MVP (egui): canvas, pannelli, strumenti, file | ✅ 7 test logici, MANUAL_TEST.md |
| Maschere — canale alpha applicato al layer, su CPU e GPU | ✅ 11 test, `mask_demo.png` |
| Selezione — rettangolo, ellisse, lazo → `Mask`, con antialiasing | ✅ 10 test, `selection_demo.png` |
| Layer ops — duplica, fondi, appiattisci, raggruppa, separa, riordina | ✅ 19 test, `groups_demo.png` |
| Integrazione `draw`/`text` → `render` (tessera ritagliata, blend, parità) | ✅ 7 test di parità, `stroke_on_layer.png` |
| Modulo A — effetti: 3 point-wise + 3 a kernel, catene, parità CPU/GPU | ✅ 6 effetti, `effects_demo.png` |
| `docs/spec/` — inventario schermate, catalogo effetti, glossario | ✅ generato e verificato |
| Crop/resize, forme, gradienti come maschera | ⏳ prossimo |
| Modulo 3D | ⏳ |

### Interfaccia: l'MVP che scopre i buchi

```bash
cargo run --offline --release -p pizzart-ui
```

egui con backend **glow** (OpenGL): il renderer del motore usa wgpu 23, e una
seconda copia di wgpu solo per disegnare i widget sarebbe stata compilazione
sprecata. La finestra si apre, il canvas mostra il composito, i pannelli
modificano il documento.

La scelta che conta: **la logica non conosce egui**. `Editor` (in
`crates/ui/src/state.rs`) contiene documento, selezione, strumento, drag e
history; `PizzartApp` è il guscio che disegna widget e chiama quei metodi. Così i
comportamenti sbagliati si verificano senza aprire una finestra:

```
la_history_con_un_drag_ha_un_solo_comando  100 passi di slider → 1 comando
un_drag_senza_movimento_non_sporca_la_history
selezione_rect_produce_mask                dentro opaca, fuori nascosta
il_composito_viene_aggiornato_dopo_undo    needs_recompose dopo l'undo
carica_png_come_layer / salva_progetto_roundtrip_da_ui
```

Il buco più probabile — 60 comandi al secondo trascinando lo slider di opacità —
è chiuso dal pattern: `begin_opacity_drag` ricorda il valore, `update` modifica il
documento **senza history**, `end` registra **un** comando. Il test lo verifica
con 100 passi.

Cosa ho verificato davvero e cosa no: il binario si compila in release, si avvia
e resta vivo senza errori su stderr; **l'aspetto e l'interazione vanno provati a
mano** con la checklist in [`MANUAL_TEST.md`](MANUAL_TEST.md). In questa sessione
non posso vedere né cliccare una finestra.
### Salvataggio: un file che si regge da solo

```
progetto.pizzart          (zip)
├── manifest.json          schema, document, nodes (con uuid), assets
└── assets/<uuid>.bin      pixel e maschere, compressi Zstd
```

Il manifest resta piccolo e ispezionabile anche con un documento da centinaia di
megabyte: `unzip -p progetto.pizzart manifest.json`.

**Identificativi.** In memoria `LayerId` è un contatore (veloce, ma due sessioni
numerano diversamente). Nel file c'è uno `StableId` a 16 byte con forma UUID v4,
generato da uno xorshift seminato con tempo, contatore e indirizzo — **zero
dipendenze** nel core. Al load il contatore riparte, l'id stabile no: è ciò che
permette copia-incolla tra progetti e template referenziabili.

**Compressione.** Zstd livello 1 sui pixel RGBA8 (si comprimono bene), `Raw` per
contenuti già compressi. Lo zip è solo il contenitore, non comprime due volte.

```
640×400, 4 layer (gruppo con maschera + testo):  48 KB
  save 12 ms, load 66 ms, composito identico (0/255)
1920×1080 × 5 layer, release, minimo di 3:      ~35 MB, save ~0,6 s, load ~0,4 s
```

Due cose che il formato fa e una che non fa:

- **Maschere condivise restano condivise.** Due layer che puntano allo stesso
  `Arc<Mask>` producono **un solo** asset; al load tornano a condividere lo
  stesso `Arc` (verificato con `Arc::ptr_eq`).
- **Le radici si ricavano dai riferimenti**: un nodo è radice se nessun altro lo
  elenca come figlio. Niente campo "root" da tenere sincronizzato.
- **I font non sono nel formato**, e non per dimenticanza: il testo oggi è
  rasterizzato in una tessera, quindi nessun layer riferisce un font. Il manifest
  ha già `font_strategy` e `fonts` perché il giorno in cui esisterà un layer di
  testo il formato non debba cambiare versione. Test 9, 10 e 14 della specifica
  (font reference/embed) restano quindi **non implementati**, e questa è la
  ragione tecnica, non una scorciatoia.

### Export: dal buffer a un file

`pizzart-io` fa una cosa sola: trasforma un'immagine composta in **byte** di un
formato. Non scrive su disco — chi chiama decide se quei byte vanno in un file,
in una clipboard o in un upload.

```rust
encode(&img, ExportFormat::Png, &ExportOptions::default())?;
encode(&img, ExportFormat::Jpeg { quality: 90 }, &opzioni)?;
encode(&img, ExportFormat::WebP { quality: 90, lossless: true }, &opzioni)?;
```

Le regole che non si vedono a occhio:

- **PNG** conserva l'alpha; il fondo viene ignorato.
- **JPEG** non ha alpha. Se l'immagine è trasparente e non è stato dato un fondo,
  è un **errore**: un colore scelto dalla libreria sarebbe una decisione di
  prodotto presa di nascosto. Se l'immagine è già opaca, il fondo non serve.
- Le qualità stanno in `1..=100`; `0` e `101` sono errori espliciti.

```
round trip PNG          bit per bit identico
round trip JPEG q95     max 5/255 (tolleranza dichiarata)
1920x1080, release, min di 3:
  PNG    10-135 ms   (dipende dal contenuto)
  JPEG   ~330 ms     (~6,5 MP/s, indipendente dal contenuto)
```

![Il documento di prova esportato in PNG](export_demo.png)

Due deviazioni dichiarate, misurate invece che indovinate:

- **JPEG 1920×1080 in 330 ms, non < 100 ms.** L'encoder disponibile
  (`zune-jpeg`, monothread) va a ~6,5 MP/s su questa CPU: 2 MP sono 320 ms. Il
  numero della specifica era un'assunzione; il test ora verifica quello misurato
  (600 ms con margine), e il modo di migliorarlo è noto (parallelizzare i blocchi
  o cambiare encoder), non un mistero.
- **WebP con perdita non è supportato**: `image-webp` 0.2 codifica solo in
  lossless. Restituire un file lossless chiamato lossy sarebbe peggio di un
  errore chiaro, quindi `WebP { lossless: false }` risponde
  `FeatureNotEnabled`. L'API è già pronta per un encoder lossy quando arriverà.

I metadati (`software`, `author`, `copyright`) sono accettati e conservati ma
**non ancora scritti**: gli encoder attuali non espongono i chunk testuali. Un
campo inerte e dichiarato è meglio di un supporto finto.

### Layer ops: l'albero

`Document.layers: Vec<Layer>` è diventato `Document.root: Vec<Node>`, dove un
nodo è un layer **o** un gruppo. È il cambio strutturale che sblocca il vero
workflow multi-layer, ed è stato fatto una volta sola, prima di costruirci sopra
crop, export e salvataggio.

```rust
enum Node { Layer(Layer), Group(Group) }
doc.layers()          // vista piatta, per chi disegna o esporta
doc.merge_down(top)   // Result<(), LayerError>
doc.flatten_group(id) // Result<LayerId, LayerError>
doc.duplicate(id)     // Result<LayerId, LayerError>
doc.create_group(&ids) -> Result<LayerId, LayerError>
doc.ungroup(id) / doc.reorder(id, target, InsertWhere::Before|After|Inside)
```

Un gruppo applica **sempre** i propri attributi al composito dei figli: niente
"passa attraverso". Blend è "fuori" (fonde il gruppo con il resto), opacità e
maschera sono "dentro" (scalano l'alpha del gruppo). La distinzione non è
estetica: senza, il risultato dipenderebbe dall'ordine dei fratelli.

Sulla GPU un gruppo è **una sorgente come le altre**: si compone in una texture
propria e poi si disegna con `draw_source`, esattamente come un layer. Lo shader
non sa che esiste un concetto di gruppo — e non deve. Costo: una texture piena
per gruppo (64 MB su 4000×4000), documentato e da ottimizzare quando servirà.

Tre dettagli che hanno richiesto attenzione:

- **`AddLayer` non può consumare il layer**: serviva al redo. Ora conserva il
  proprio layer e inserisce una copia; `RemoveLayer` gestisce il caso "già
  rimosso", altrimenti il secondo redo non faceva nulla.
- **`MergeDown` cuoce la maschera del layer sotto** nei pixel e la azzera: se
  restasse, sarebbe applicata due volte (una nella fusione, una al compositing).
- **`FlattenGroup` resetta il transform**: i pixel del composito sono già nello
  spazio del documento; conservare la traslazione li sposterebbe due volte.

![Piatto, gruppo Multiply, appiattito](groups_demo.png)

Pannelli 1 e 3 sono identici entro **1/255**: l'appiattimento è fedele al
composito. Il pannello 2 è diverso (**77/255**) perché il blend del gruppo agisce
su tutto il gruppo, non sui singoli layer.

### Selezione: il terzo produttore di maschere

La selezione non ha toccato né il tipo `Mask` né il compositing: ha aggiunto
**un modo in più di produrre alpha**. Era il punto del fare prima le maschere.

```
qualsiasi alpha → Mask → layer
  ├── Mask::from_tile / from_tile_placed   (contenuto: testo, tratti, immagini)
  ├── Mask::full / empty                   (neutro)
  └── RectSelection / EllipseSelection / LassoSelection   (questo passo)
```

La rasterizzazione è **una sola funzione**: ogni forma fornisce "questo punto è
dentro?", e `rasterize_alpha` campiona 4×4 per pixel. Rettangolo, ellisse e
lazo hanno quindi gli stessi bordi, e le forme — che verranno dopo — riusano la
stessa funzione cambiando solo il tipo di ritorno.

```
rettangolo: w/h negativi normalizzati (drag in qualsiasi direzione)
ellisse:    w o h = 0 → maschera vuota, non panico
lazo:       < 3 punti → vuota; riempimento even-odd (regola prevedibile
            per una selezione disegnata a mano); poligono sempre chiuso
bordi:      4×4 campioni/pixel → un bordo a metà pixel vale ~128
maschera:   sempre delle dimensioni del documento, anche se la selezione è in un angolo
```

Un caso che vale la pena raccontare: `ellisse_cerchio_al_centro` è fallito la
prima volta. Con centro intero e raggio 50 il bordo cade **esattamente** sulla
griglia dei pixel, quindi il pixel 150 è interamente fuori — e il test che
pretendeva lì un valore intermedio aveva torto. Ho corretto il test (e aggiunto
il caso con centro a metà pixel, dove l'antialiasing è misurabile), non il
codice.

![Modifica locale: rettangolo e cerchio](selection_demo.png)

### Maschere: un canale alpha è una risorsa

Una maschera non è contenuto: è un **modificatore**. Non ha colore, non ha blend,
non produce pixel visibili. Ha un solo numero per pixel — quanto si vede — e
questa semplicità è ciò che la rende componibile.

```
qualsiasi sorgente di alpha → Mask → layer
```

Font, tratti, immagini, selezioni, gradienti: producono tutti la stessa
maschera, e la pipeline di compositing non cambia. È l'ortogonale del primo
pattern (`bytes → registro → handle`): **contenuto** contro **modificatore**.

```
CPU:   opacity_effettiva = layer.opacity × mask.sample(u, v)
GPU:   binding 4 = texture R8; una maschera 1×1 bianca quando il layer non ne ha
parità: mask semplice / invertita / da testo, max 2–3/255
undo:  SetLayerMask e ToggleMaskInverted sono comandi reversibili
```

Due decisioni dichiarate:

- **Una maschera di dimensioni diverse viene adattata** (nearest, clamp), non
  rifiutata: campionarla con le UV del layer è il comportamento che serve a chi
  usa un testo o una forma come maschera. Il caso 1:1 resta banale.
- **L'inversione si applica al caricamento** (`255 - alpha`), non nel shader: la
  GPU riceve sempre "quanto si vede", e lo shader non ha un ramo in più.

![Il testo come maschera su un gradiente](mask_demo.png)

### Testo: una risorsa esterna, un pattern

Il testo non è "un'altra feature": è la quarta sorgente di tessere, e la prima
che non nasce da pixel o da punti del tablet.

```
bytes → validazione → registro → handle → PlacedImage → GPU
```

`pizzart-text` non sa da dove arrivano i byte: file, rete o memoria sono
responsabilità di chi carica. La libreria valida e rasterizza. È il caso più
semplice e reale del pattern "risorsa esterna", e serve a capire se regge prima
di estenderlo a sticker, LUT e pennelli.

```
registro: max 20 MB per font (DoS), firma TTF/OTF/TTC, 256 font,
          64 volti per collection, niente duplicati di famiglia
layout:   shaping completo (cosmic-text): kerning, legature, BiDi,
          fallback dei glifi, wrapping, allineamento
tessera:  ritagliata all'ingombro — «PIZZART» a 104 px = 124 KB, non 1,8 MB
parità:   testo CPU vs composizione GPU, max 3/255
```

Nel renderer **non è servito codice nuovo**: `upload_stroke` è diventato
`upload_tile` (con alias per non rompere nulla), perché un tratto e un testo
sono la stessa cosa per la GPU — una tessera RGBA8 con una posizione.

![Testo composto su GPU](text_demo.png)

Una deviazione dichiarata: `Text::render` prende `&mut FontRegistry` e non
`&FontRegistry`. cosmic-text muta lo stato interno durante lo shaping (cache dei
glifi, fallback): fingere immutabilità avrebbe richiesto interior mutability
solo per far tornare la firma scritta nella specifica.

### Effetti: infrastruttura prima del contenuto

Un effetto è una descrizione, non un'operazione: `Effect { shader, params, blend,
opacity }`. Il renderer lo applica come **pass separato** in ping-pong su due
texture; lo shader non conosce gli altri effetti né il blend. L'ingresso non
viene mai modificato.

Quattro effetti validano due pipeline diverse:

| Effetto | Tipo | Pass |
|---|---|---|
| Luminosità, Contrasto, Saturazione | point-wise | 1 |
| Sfocatura gaussiana | kernel separabile | 2 (orizzontale + verticale) |
| Nitidezza (unsharp mask) | kernel + combinazione | 3 |
| Bagliore | kernel + combinazione con Screen | 3 |

I restanti 113 del catalogo si aggiungono uno per volta con lo stesso schema:
uno shader, una variante di `EffectShader`, un test di parità.

```
parità CPU/GPU, singolo effetto       max 2/255
parità CPU/GPU, blur (raggi 1-8)      max 1/255
parità CPU/GPU, sharpen / glow        max 2/255 con Multiply e opacità 0,5
catena di 4 effetti                   max 4/255   (ogni pass quantizza a 8 bit)
catena blur+sharpen+glow              max 5/255   (8 pass in tutto)
blur raggio 8 su 512x512              11,1 ms     (minimo di 5, release)
  di cui costo fisso (pass + readback) 3,8 ms
  di cui kernel                       7,3 ms
  prima chiamata                      ~90 ms     (compilazione pigra del pipeline)
```

Tre scelte che vale la pena conoscere:

- **Blend e opacità si applicano una volta sola**, nell'ultimo pass. Il flag
  `is_last` è un campo dell'uniform, non un parametro dell'effetto: senza, un
  blur con `Multiply` moltiplicherebbe due volte (una per pass). Con `Normal` e
  opacità 1 non si vedrebbe, ed è esattamente il tipo di bug che preferisco non
  avere.
- **Tre texture, non due.** Gli effetti a combinazione (nitidezza, bagliore)
  devono confrontare il risultato del kernel con l'originale: la terza texture
  tiene l'originale intatto mentre le altre due fanno ping-pong. Il pass di
  combinazione ha un layout con due ingressi.
- **Il campionamento è nearest**, non bilineare. Il bilineare è più bello ma non
  avrebbe una CPU reference con cui confrontarsi: qualità senza contratto è
  debito. Arriverà quando ci sarà un riferimento anche per la filtrazione.

Il riferimento CPU (`pizzart_render::cpu`) non è codice di test: serve davvero
all'export senza GPU e documenta cosa deve fare ogni effetto.

![Sei pannelli: originale, luminosità, catena, sfocatura, nitidezza, bagliore](effects_demo.png)

### Solo dove serve: dirty rect

Un tratto occupa una regione piccola; allocare la tela intera è sprecato. Ogni
tratto ora produce un `PlacedImage` — pixel più posizione — e la tessera viaggia
fino alla GPU:

```
tratto 100x100 su tela 4000x4000 (release):
  prima  ~200 ms, 64 MB allocati e caricati per ogni pennellata
  oggi   1,328 ms, 50 KB
```

La composizione usa la posizione della tessera (matrice di traslazione), non la
tela intera. `Stroke::render` resta disponibile come wrapper lento per
compatibilità, ma il percorso veloce è `render_bbox`. Il tipo `PlacedImage` vive
nel core perché gli effetti e le selezioni hanno lo stesso problema.

### Dal tablet alla GPU

La catena completa è verificata: i punti del tablet diventano timbri su CPU
(`pizzart-draw`), il buffer RGBA8 diventa una texture, e `pizzart-render` lo
compone sopra i layer del documento con il blend mode del pennello.

```
scena 900x620 (sfondo + cerchio + 3 tratti)   differenza CPU/GPU: max 2/255
matrice dei 12 blend modes su grigio          differenza CPU/GPU: max 1/255
```

Quattro test di parità coprono il ponte: uno stroke, tutti e 12 i modi insieme,
il moltiplicatore di opacità, e l'ordine di disegno. Se la GPU diverge dalla
CPU, il test fallisce — è il contratto che protegge dagli errori silenziosi.

### Motore di disegno (CPU)

Uno stroke di 1000 punti su tela 512×512 richiede **6,59 ms** su un thread
(traguardo: < 50 ms). La demo `stroke_demo.png` disegna 6 tratti con 6 modi di
fusione, 675 timbri totali su 960×640 in 86 ms.

Il costo è proporzionale alla **tela**, non al tratto: ogni stroke alloca una
maschera e un buffer delle dimensioni del documento. Il prossimo passo ovvio è
limitare tutto al riquadro d'ingombro del tratto.

### Parità CPU/GPU

Il contratto del renderer è semplice: la GPU deve produrre gli stessi pixel del
riferimento CPU. Su un documento di prova (800×500, 7 layer, 6 blend modes):

```
CPU composite :   106.14 ms
GPU render    :   101.39 ms   (upload + pass + readback, prima iterazione)
differenza CPU/GPU: max 2/255, media 0.020/255
```

La differenza di 2/255 è l'arrotondamento float→unorm8, non un bug. I tempi
sono la prima misura, non un traguardo: caricare una texture per layer e
rileggere il risultato dominano il costo, e si eliminano con tiling e pass
multipli (obiettivo 5–15 ms).

## Struttura

```
pizzart/
├── Cargo.toml              workspace
├── crates/
│   ├── core/               documento, layer, fusione, storia (zero dipendenze)
│   ├── draw/               motore di disegno: pressione, tilt, spazzole, spline
│   └── render/             wgpu: compositing GPU
├── docs/spec/              specifiche DERIVATE dall'analisi (generate, non scritte a mano)
└── tools/                  generatori riproducibili delle specifiche
```

## Come si esegue

```bash
cargo test --offline                    # tutto il progetto, senza rete
cargo test --offline -p pizzart-core    # solo il core, istantaneo
```

Il core non ha dipendenze esterne: si compila e si testa su qualsiasi macchina,
anche offline, anche in CI senza cache. Solo `pizzart-render` richiede `wgpu`.

## Le specifiche

`docs/spec/` contiene tre documenti **generati** dai dati dell'app analizzata:

| Documento | Cosa contiene | Da dove viene |
|---|---|---|
| `schermate.md` | 243 schermate, 13 deeplink, aree funzionali, mappa delle rotte | `AndroidManifest.xml` |
| `effetti.md` | 119 effetti + 109 parametri + 72 enum, con nomi EN/IT | tabella di localizzazione degli effetti |
| `glossario.md` | vocabolario UI minimo EN → IT | tabella delle stringhe (3.247), filtrata |

Il materiale grezzo dell'analisi **non entra nel repository**: resta in
`../_estrazione/` (ignorata da git) e i generatori lo leggono in sola lettura.

```bash
python tools/estrai_input.py            # estrae i 3 input dagli archivi
python tools/gen_spec.py                # rigenera docs/spec/
```

Perché è importante: le specifiche sono **fatti derivati** (nomi di schermate,
catalogo di effetti, terminologia), non codice né risorse dell'app. Il
comportamento — formule, valori, layout, asset — è progettato da zero e testato
in questo repository.

## Posizione sul progetto originale

- Il codice di Picsart non è stato tradotto né riusato: 71k file Java, di cui la
  maggior parte pubblicità e 24.397 classi offuscate, non contengono un motore di
  editing riusabile.
- L'analisi serve come **capitolato funzionale**, la stessa funzione del team
  "dirty" in una clean room.
- Non si riusano asset (icone, font, texture), non si replica l'interfaccia
  pixel per pixel, non si ricostruisce il backend.
- Le aree fuori perimetro (pubblicità, abbonamento, social, store, mini-app)
  sono elencate nell'inventario e non implementate.

## Metriche di riferimento

| Operazione | Obiettivo |
|---|---|
| Apertura editor | < 100 ms |
| Applicare un filtro | 5–15 ms |
| Undo di 50 passi | istantaneo |
| Latenza del disegno | < 5 ms |
| Export 4K | 1–3 s |
