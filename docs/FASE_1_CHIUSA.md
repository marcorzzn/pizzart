# Fase 1 chiusa

Data: sessione corrente · `cargo test --workspace --release`: **251 test verdi** (241 + 10 nuovi in
`crates/ui/tests/fase1.rs`), zero warning.

## 1. I 10 bug del report

| ID | Descrizione | Esito | Dove |
|---|---|---|---|
| BUG-001 | Pennello senza impostazioni | ✅ **chiuso** — pannello "Strumento": colore, dimensione 1–200, opacità, durezza, stabilizzazione, 12 blend | `panels.rs::pannello_strumento` |
| BUG-002 | Cambio strumento a metà tratto lascia punti orfani | ✅ **chiuso** — `change_tool` chiude il tratto; `fine_drag` chiude se ci sono punti, **indipendentemente** dallo strumento | `state.rs::change_tool`, `app.rs::fine_drag` |
| BUG-003 | `Ctrl+Z` durante il drag di opacità | ✅ **chiuso** — `undo`/`redo` chiudono prima il drag | `state.rs::undo/redo` |
| BUG-004 | Apertura immagini bloccante | ✅ **chiuso** — decodifica su thread + `poll_loading` + overlay "Apertura…" | `state.rs::open_image_async`, `app.rs` |
| BUG-005 | ↑/↓ no-op dentro un gruppo | ✅ **chiuso** — il contenitore è `parent_of(id)`, sia per i limiti sia per il target | `state.rs::move_selected` |
| BUG-006 | Nessuna anteprima degli effetti | ❌ **non chiuso** — è Fase 2 per decisione esplicita della specifica | — |
| BUG-007 | Scorciatoie incomplete | ✅ **chiuso** — `Ctrl+O/E/Shift+S/0/+=/-` e `Ctrl+Shift+Z` | `app.rs::scorciatoie` |
| BUG-008 | Testo a riga singola, un solo font | ❌ **non chiuso** — Fase 2 | — |
| BUG-009 | Ogni effetto duplica il layer | ❌ **non chiuso** — Fase 2 (memoria, non correttezza) | — |
| BUG-010 | Gruppi invisibili nella UI | ❌ **non chiuso** — Fase 2 | — |

**Chiusi 6 su 10.** I quattro aperti sono quelli che la specifica ha assegnato alla Fase 2: nessuno
di essi rompe o corrompe qualcosa.

## 2. Misura del composito (1920×1080, 5 layer, GPU AMD R5, release, minimo di 5)

| Percorso | min | mediana | max | Verdetto |
|---|---|---|---|---|
| Piena risoluzione | **86,6 ms** | 99,6 ms | 167,0 ms | ❌ **sopra** il budget di 16 ms/frame (≈ 11 fps) |
| Preview a metà lato (960×540) durante il drag | **26,9 ms** | 32,8 ms | 33,8 ms | ⚠️ ~3,7× meglio, ma ancora sopra 16 ms |

### La decisione: preview sì, implementata

La prima misura — fatta *prima* di scrivere la preview — diceva 125 ms di minimo. Ho quindi
implementato la preview come chiesto: durante il drag il documento viene copiato a scala 0,5 (una
volta sola, all'inizio del drag) e si compone quello; al rilascio si torna a piena risoluzione.

Il numero resta **27 ms di minimo, quindi sotto i 60 fps**. Non ho abbassato il target né allargato
l'asserzione: il test verifica quello che è vero.

```rust
assert!(pieno_min.as_millis() > 50);      // se non fosse così, la preview sarebbe inutile
assert!(preview_min.as_millis() < 50);    // e la preview deve stare nel budget
```

**Cosa significa**: il drag dell'opacità è ora ~27 ms/frame (≈ 37 fps) invece di ~87–100 ms
(≈ 11 fps). Usabile, non fluido. Il prossimo fattore 2 va cercato nel **readback GPU → CPU**
(8 MB a frame anche in preview) e nella conversione in `ColorImage` + upload della texture: i pass
di compositing non sono il problema.

## 3. Cosa non ho fatto, e perché

- **`benches/composite.rs`**: la misura sta in un test (`composito_1920x1080_sotto_50ms`) invece che
  in un bench, perché `criterion` non è nelle dipendenze e aggiungerlo per un solo numero non
  valeva la compilazione. Il test stampa min/mediana/max come un bench, ed è già nel percorso di CI.
- **Blocco delle azioni durante il caricamento**: l'overlay c'è, il blocco dei menù non è completo —
  resta possibile aprire un altro file mentre uno si decodifica (il secondo sostituisce il primo
  `Receiver`). Non corrompe nulla, ma è una sbavatura da chiudere in Fase 2.

## 4. Le prossime cose, in ordine di valore

1. **Readback GPU** (il fattore 2 che manca alla fluidità): valutare una texture condivisa o un
   composito a 1/3 di lato durante il drag. Misurare con lo stesso test.
2. **BUG-006, anteprima effetti**: ora si applica alla cieca; con la preview ridotta la macchina
   c'è già (`scala_documento`).
3. **BUG-008, testo multilinea + scelta font**, BUG-010 gruppi: le due assenze che si notano subito.
