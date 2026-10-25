# Fase 2 — specifica tecnica (bozza, non committata)

Due lavori, in quest'ordine. Il primo è un numero da migliorare, il secondo è un'assenza da colmare.
Nessuno dei due tocca il motore: sono entrambi dentro la UI e il suo rapporto col renderer.

---

## 1. Readback GPU: il fattore 2 che manca alla fluidità

### Il numero misurato

| Percorso | min | mediana | max |
|---|---|---|---|
| Composito pieno 1920×1080 × 5 layer | **86,6 ms** | 99,6 ms | 167,0 ms |
| Preview a metà lato (960×540) durante il drag | **26,9 ms** | 32,8 ms | 33,8 ms |

La preview a metà lato **dimezza i pixel ma non i tempi**: dovrebbe costare un quarto, costa un
terzo. Questo dice dove non è il costo.

### Da cosa è composto un frame (ipotesi da verificare)

```
renderer.render(&doc)              // 5 pass GPU + upload delle texture dei layer
  → read_rgba()                    // texture → buffer → map_async → CPU   [8 MB]
  → ColorImage::from_rgba_unmultiplied
  → TextureHandle::set()           // CPU → VRAM, 8 MB                      [8 MB]
```

Il sospetto principale è che **passino 16 MB di pixel per frame** fra due trasferimenti
(GPU→CPU e CPU→GPU), quando la GPU ha già disegnato tutto: l'immagine viene scaricata solo perché
egui (backend glow) sa disegnare soltanto texture che ha caricato lui.

L'altro sospetto è `map_async` + `device.poll(Maintain::Wait)`: è una **sincronizzazione** che
attende il completamento completo della coda. Non è latenza di trasferimento, è latenza di
pipeline: ogni frame paga il costo di svuotare la coda.

### Esperimenti, in ordine di costo

1. **Misurare le due metà separate** (mezz'ora): cronometrare `render()` e `read_rgba()` dentro
   `composite_rgba` su 100 iterazioni, senza la conversione egui. Se il render è 20 ms e il readback
   60, la strada è decisa prima di scrivere una riga.
   *Questo va fatto per primo: senza, il resto è scommessa.*
2. **Campionamento dimezzato senza ricomporre**: invece di comporre a 960×540 (che costa un upload
   di 5 texture piccole + 5 pass), comporre a piena risoluzione **una volta** e poi leggere solo i
   pixel pari (1/4 dei byte). Se il tempo scende di ~3/4 significa che il costo è nel trasferimento,
   non nei pass. Costo: un'ora, ed è reversibile.
3. **Doppio buffer asincrono** (il vero fix, se serve): `read_rgba` non deve bloccare il frame.
   - due buffer mappabili, alternati;
   - a ogni frame si chiede il readback del frame precedente e si disegna quello (un frame di
     ritardo);
   - `Maintain::Poll` invece di `Wait`.
   - Effetto: il costo della sincronizzazione sparisce, resta il trasferimento.
4. **Composito a 1/3 di lato durante il drag** (se dopo 2–3 il numero è ancora sopra 20 ms):
   640×360 = 1/9 dei pixel. La preview è più morbida ma resta leggibile per giudicare un'opacità.

### Contratto di accettazione

- `composito_preview` sotto **16 ms** in release, minimo di 5, con lo stesso documento del test
  attuale (`crates/ui/tests/fase1.rs::composito_1920x1080_sotto_50ms`, da rinominare).
- Il composito pieno **non deve peggiorare**: resta il test che registra min/mediana/max.
- Nessuna regressione di correttezza: il canvas a riposo deve essere bit-identico a `doc.composite()`.
- Il test attuale asserisce `pieno_min > 50` come "la preview serve": va aggiornato, altrimenti
  diventa falso il giorno in cui il pieno scende sotto i 50 ms.

### Rischio da mettere in conto

Con il backend glow, condividere la texture wgpu con egui **non è possibile** senza un ponte
(`egui_wgpu::CallbackTrait`) che richiede il renderer su wgpu per la UI. Passare la UI a
`eframe/wgpu` significherebbe due copie di wgpu nello stesso processo (versioni diverse) oppure
allineare la versione di wgpu del motore a quella di eframe: **quest'ultima è la strada** se il
doppio buffer non basta (wgpu 23 → quella richiesta da eframe 0.29). Da valutare solo se i punti
2–4 non bastano: è un cambio di dipendenza, non un fix.

---

## 2. Anteprima degli effetti

### Il problema

`panels.rs::pannello_effetti` legge lo slider solo al click su "Applica": l'utente applica alla
cieca. Il motore ha già tutto (`renderer.apply_effects`), manca il collegamento.

### Il design

```
drag_start  → crea un layer temporaneo "anteprima" sopra il selezionato,
              a risoluzione ridotta (stesso `scala_documento` della Fase 1)
drag        → aggiorna i parametri del layer temporaneo (non tocca il documento,
              non crea comandi di history)
drag_end    → rimuove il temporaneo:
              "Applica"   → apply_effect sul layer vero (un comando di history)
              "Annulla"   → scarta
```

Dettagli che decidono la riuscita:

- **Il temporaneo è un `Layer` vero**, aggiunto a `Document.root` ma marcato (`name: "anteprima"`,
  un flag nel pannello, non nel motore). Così la composizione usa il percorso normale e non serve
  un canale di rendering separato.
- **La maschera del layer originale va copiata** sul temporaneo, altrimenti l'anteprima di un layer
  mascherato mostra l'effetto dove non si vedrà.
- **La risoluzione**: riusare `scala_documento(0.5)` sulla sola tessera dell'effetto, non sul
  documento. L'effetto è locale: scalare tutto il documento sarebbe sproporzionato.
- **Costo per frame**: `apply_effects` su una tessera a metà è nell'ordine dei 5–15 ms per blur
  raggio 8 su 512² (misurato 11,09 ms a piena risoluzione). Quindi l'anteprima va **solo al cambio
  di valore**, non a ogni frame: si ridisegna quando lo slider cambia, e non mentre è fermo.
- **Nessun comando di history** durante il drag: la regola di `begin_opacity_drag` vale anche qui,
  e il test `la_history_con_un_drag_ha_un_solo_comando` è il modello da replicare con
  `la_history_con_un_anteprima_ha_un_solo_comando`.

### Contratto di accettazione

- Muovendo lo slider, il canvas cambia **senza** che la history cresca.
- Al rilascio: "Applica" → 1 comando; "Annulla" → 0 comandi; in entrambi i casi il temporaneo non
  resta nel documento.
- L'anteprima di un layer con maschera rispetta la maschera.
- Con 5 layer a 1920×1080, l'anteprima non deve superare i 50 ms per aggiornamento (misurato).

---

## Ordine e stima

| # | Lavoro | Stima | Dipende da |
|---|---|---|---|
| 1 | Misura separata render/readback | 0,5 g | — |
| 2 | Campionamento dimezzato in lettura | 1 g | 1 |
| 3 | Doppio buffer asincrono | 1–2 g | 1, 2 |
| 4 | Anteprima effetti (design sopra) | 1–2 g | 3 (riusa la preview ridotta) |
| 5 | Test e aggiornamento del contratto di fluidità | 0,5 g | 3, 4 |

**Prima di tutto il resto**: la verifica manuale della Fase 1. Se il drag è già usabile a ~37 fps
sull'hardware dell'utente, i punti 2–4 valgono meno; se scatta, la priorità torna al punto 1.
