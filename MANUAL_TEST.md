# Pizzart — test manuale

La UI non si verifica headless: quello che segue è la checklist per provarla a
mano. I comportamenti logici (history con lo slider, selezione → maschera, undo
che invalida il canvas) hanno già test automatici in `crates/ui/tests/`.

## Avvio

```bash
cargo run --offline --release -p pizzart-ui
```

Deve aprirsi una finestra 1280×800 con:
- menù **File / Modifica / Vista** e i quattro strumenti in alto;
- canvas scuro centrale;
- pannello destro con **Layer** ed **Effetti**;
- barra di stato in basso (dimensioni, zoom, layer, GPU, messaggi).

## Percorso minimo (i 6 passaggi che chiudono l'MVP)

| # | Azione | Cosa deve succedere |
|---|---|---|
| 1 | File → Apri… → un PNG | Il canvas mostra l'immagine, un layer compare nel pannello, la barra di stato mostra le dimensioni |
| 2 | Strumento Pennello (B), trascina sul canvas | Il tratto compare; al rilascio diventa un layer "tratto" |
| 3 | Strumento Testo (T), clicca, scrivi, OK | Il testo compare nella posizione del click |
| 4 | Effetti → Luminosità +0,3 → Applica | Il layer selezionato viene schiarito (diventa un layer "effetto su …") |
| 5 | Ctrl+S | Chiede dove salvare; un `.pizzart` viene scritto |
| 6 | File → Esporta PNG… | Il composito viene scritto su disco e si apre nel visualizzatore |

Se questi sei passaggi funzionano senza crash, l'MVP è chiuso.

## Comportamenti da verificare

### Canvas
- [ ] Rotella = zoom, centrato sul cursore (il punto sotto il mouse resta fermo)
- [ ] Zoom limitato fra 5% e 800%
- [ ] Drag con tasto **centrale** = pan
- [ ] **Spazio + tasto sinistro** = pan
- [ ] Vista → Adatta riporta zoom 100% e pan a zero

### Layer
- [ ] Click su un layer nel pannello lo seleziona
- [ ] Checkbox di visibilità nasconde/mostra (e l'undo lo ripristina)
- [ ] Slider opacità: **un solo** passo di undo per un trascinamento lungo
- [ ] Combo blend mode cambia il modo di fusione
- [ ] ↑ / ↓ spostano il layer nell'ordine
- [ ] "Fondi" fonde il layer con quello sotto (l'ordine resta coerente)
- [ ] "Elimina" toglie il layer e l'undo lo riporta

### Strumenti
- [ ] S / B / T / R cambiano strumento (solo quando non si scrive in un campo)
- [ ] Seleziona: click su un punto trasparente non seleziona nulla
- [ ] Seleziona: click su un pixel opaco seleziona quel layer
- [ ] Pennello: un click senza trascinamento non crea layer vuoti
- [ ] Selezione rettangolo: drag mostra il bordo azzurro; **Invio** applica la maschera; **Esc** annulla
- [ ] Selezione troppo piccola (pochi pixel) viene scartata

### Maschere
- [ ] Dopo "Maschera ellittica" il layer mostra il simbolo ◐ nel pannello
- [ ] "Togli maschera" riporta l'alpha pieno
- [ ] L'undo ripristina la maschera

### Effetti
- [ ] Tutti e sei gli effetti applicano qualcosa di visibile
- [ ] Con layer non selezionato il bottone "Applica" è disabilitato
- [ ] L'undo toglie il layer d'effetto (l'originale è intatto)

### File e history
- [ ] Annulla/Ripeti funzionano anche dopo catene (pennello + testo + effetto + opacità)
- [ ] Salva senza percorso apre il dialogo "Salva con nome"
- [ ] Riaprendo il `.pizzart` il composito è identico
- [ ] Aprire un file non valido mostra il dialogo di errore, non chiude l'app

### Cose che il primo giro **non** deve dare per scontate
- [ ] Trascinare lo slider di opacità su un documento grande (1920×1080, 5 layer) non deve bloccare l'interfaccia per più di ~0,1 s per frame
- [ ] Se scatta, è il segnale che serve la preview a bassa risoluzione durante il drag

### Pennello (Fase 1)

- [ ] Con lo strumento Pennello il pannello mostra colore, dimensione, opacità, durezza, stabilizza, fusione
- [ ] Cambiando colore, il tratto esce di quel colore
- [ ] Cambiando dimensione (1–200), il tratto cambia spessore
- [ ] Opacità a 0,3 rende il tratto traslucido
- [ ] Cambiare strumento **a metà tratto** non lascia un segmento a sorpresa nel tratto dopo

### Scorciatoie (Fase 1)

- [ ] Ctrl+O apre, Ctrl+E esporta PNG, Ctrl+Shift+S salva con nome
- [ ] Ctrl+0 adatta, Ctrl+= e Ctrl+- cambiano lo zoom
- [ ] Ctrl+Shift+Z ripete come Ctrl+Y

### Apertura non bloccante (Fase 1)

- [ ] Aprendo un'immagine grande compare "Apertura…" con lo spinner e la finestra resta reattiva

### Fluidità (Fase 1, misurata)

- [ ] Il drag dell'opacità su 1920×1080 con 5 layer gira a ~37 fps (misurato 26,9 ms/frame in preview)
- [ ] Il canvas a piena risoluzione si aggiorna al rilascio dello slider
## Cosa non c'è (di proposito)

Nessuna anteprima live degli effetti mentre si trascina lo slider, nessun
drag-and-drop dei layer, nessun tema, nessuna lingua, nessun autosave. Sono
feature del prodotto, non dell'MVP che serve a scoprire i buchi.
