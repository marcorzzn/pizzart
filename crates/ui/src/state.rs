//! Stato dell'editor: tutta la logica, senza egui.
//!
//! Questa separazione non è pedanteria architetturale: è ciò che rende la UI
//! **testabile senza finestra**. `PizzartApp` è un guscio che disegna widget e
//! chiama questi metodi; i comportamenti che contano (drag dello slider,
//! selezione → maschera, undo che invalida il canvas) si verificano headless.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use pizzart_core::history::Command;
use pizzart_core::{
    BlendMode, Document, EllipseSelection, Layer, LayerId, Mask, Node, RectSelection, Transform,
};
use pizzart_draw::{Brush, Point, Stroke};
use pizzart_io::{encode, ExportFormat, ExportOptions};
use pizzart_project::{load, save, SaveOptions};
use pizzart_render::{Effect, HeadlessRenderer};
use pizzart_text::{FontRegistry, Text};

/// Strumento attivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Brush,
    Text,
    Rect,
}

/// Un drag in corso sullo slider di opacità.
struct OpacityDrag {
    layer: LayerId,
    inizio: f32,
    corrente: f32,
}

/// L'editor: documento, renderer, selezione, strumento.
///
/// Tutte le modifiche al documento passano di qui, così la history e il flag di
/// ricomposizione restano coerenti: è l'unico punto in cui si può sbagliare.
pub struct Editor {
    pub doc: Document,
    pub registry: FontRegistry,
    pub renderer: Option<HeadlessRenderer>,
    pub path: Option<PathBuf>,
    pub selected: Option<LayerId>,
    pub tool: Tool,
    /// Il composito mostrato non è più valido.
    pub needs_recompose: bool,
    /// Ultimo messaggio per la barra di stato.
    pub status: String,
    /// Rettangolo di selezione in pixel del documento.
    pub selection: Option<(f32, f32, f32, f32)>,
    pub brush: Brush,
    pub stroke_points: Vec<Point>,
    pub zoom: f32,
    pub pan: (f32, f32),
    drag_opacita: Option<OpacityDrag>,
    /// Documento a bassa risoluzione usato durante il drag: il composito a
    /// piena risoluzione costa ~125 ms su 1920x1080, troppo per un frame.
    pub preview: Option<(Document, f32)>,
    /// Decodifica in corso su un thread: non blocca l'interfaccia.
    loading: Option<Receiver<Result<ImmagineCaricata, String>>>,
}

/// Un'immagine decodificata, pronta per diventare un layer.
pub type ImmagineCaricata = (String, Vec<u8>, u32, u32);

impl Default for Editor {
    fn default() -> Self {
        Self::new(1280, 800)
    }
}

impl Editor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            doc: Document::new(width, height),
            registry: FontRegistry::new(),
            renderer: HeadlessRenderer::new(),
            path: None,
            selected: None,
            tool: Tool::Select,
            needs_recompose: true,
            status: "documento nuovo".into(),
            selection: None,
            brush: Brush::new(24.0),
            stroke_points: Vec::new(),
            zoom: 1.0,
            pan: (0.0, 0.0),
            drag_opacita: None,
            preview: None,
            loading: None,
        }
    }

    /// Nome della GPU, se c'è.
    pub fn gpu(&self) -> &str {
        match &self.renderer {
            Some(r) => r.adapter_name(),
            None => "CPU (nessuna GPU)",
        }
    }

    // -----------------------------------------------------------------
    // File
    // -----------------------------------------------------------------

    /// Apre un'immagine e la mette come fondo del documento.
    pub fn open_image(&mut self, path: &std::path::Path) -> Result<(), String> {
        let img = image::open(path).map_err(|e| format!("apertura {}: {e}", path.display()))?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let layer = Layer::new(
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "immagine".into()),
            w,
            h,
            rgba.into_raw(),
        );
        self.doc = Document::new(w, h);
        self.selected = Some(self.doc.add_layer(layer));
        self.path = None;
        self.zoom = 1.0;
        self.pan = (0.0, 0.0);
        self.needs_recompose = true;
        self.status = format!("aperto {} ({w}×{h})", path.display());
        Ok(())
    }

    /// Apre un'immagine **senza bloccare**: decodifica su un thread worker.
    ///
    /// La decodifica di un PNG da 40 MB costa secondi: farla sul thread della UI
    /// significa una finestra congelata senza spiegazione. Qui la UI resta viva
    /// e mostra "Apertura..." finche' `poll_loading` non consegna il risultato.
    pub fn open_image_async(&mut self, path: PathBuf) {
        let (tx, rx) = channel();
        let percorso = path.clone();
        std::thread::spawn(move || {
            let esito = (|| {
                let img = image::open(&percorso)
                    .map_err(|e| format!("apertura {}: {e}", percorso.display()))?;
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                let nome = percorso
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "immagine".into());
                Ok((nome, rgba.into_raw(), w, h))
            })();
            let _ = tx.send(esito);
        });
        self.loading = Some(rx);
        self.status = format!("apertura {}...", path.display());
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    /// Da chiamare a ogni frame: consegna il risultato quando e' pronto.
    pub fn poll_loading(&mut self) -> Option<Result<(), String>> {
        let rx = self.loading.as_ref()?;
        match rx.try_recv() {
            Ok(Ok((nome, rgba, w, h))) => {
                self.loading = None;
                let layer = Layer::new(nome, w, h, rgba);
                self.doc = Document::new(w, h);
                self.selected = Some(self.doc.add_layer(layer));
                self.path = None;
                self.zoom = 1.0;
                self.pan = (0.0, 0.0);
                self.needs_recompose = true;
                self.status = format!("immagine aperta ({w}x{h})");
                Some(Ok(()))
            }
            Ok(Err(e)) => {
                self.loading = None;
                self.status = "apertura fallita".into();
                Some(Err(e))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.loading = None;
                Some(Err("decodifica interrotta".into()))
            }
        }
    }

    /// Apre un progetto `.pizzart`.
    pub fn open_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        let doc = load(path).map_err(|e| e.to_string())?;
        self.selected = doc.root.first().map(|n| n.id());
        self.doc = doc;
        self.path = Some(path.to_path_buf());
        self.needs_recompose = true;
        self.status = format!("progetto aperto: {}", path.display());
        Ok(())
    }

    /// Apre un file qualsiasi: `.pizzart` come progetto, il resto come immagine.
    pub fn open(&mut self, path: &std::path::Path) -> Result<(), String> {
        let progetto = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("pizzart"))
            .unwrap_or(false);
        if progetto {
            self.open_project(path)
        } else {
            self.open_image(path)
        }
    }

    /// Salva il progetto nel percorso dato (e lo ricorda).
    pub fn save_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        save(&self.doc, path, &SaveOptions::default()).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.status = format!("salvato {}", path.display());
        Ok(())
    }

    /// Salva nel percorso corrente, se c'è.
    pub fn save(&mut self) -> Result<bool, String> {
        match self.path.clone() {
            Some(path) => {
                self.save_project(&path)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Esporta il composito in PNG.
    pub fn export_png(&mut self, path: &std::path::Path) -> Result<(), String> {
        let (rgba, w, h) = self.composite_rgba();
        let img = pizzart_render::RenderedImage {
            width: w,
            height: h,
            rgba,
        };
        let bytes = encode(&img, ExportFormat::Png, &ExportOptions::default())
            .map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("scrittura {}: {e}", path.display()))?;
        self.status = format!("esportato {}", path.display());
        Ok(())
    }

    // -----------------------------------------------------------------
    // Composito
    // -----------------------------------------------------------------

    /// Restituisce il composito in RGBA8, preferendo la GPU.
    pub fn composite_rgba(&mut self) -> (Vec<u8>, u32, u32) {
        let base = self.preview.as_ref().map(|(d, _)| d).unwrap_or(&self.doc);
        let (w, h) = (base.width, base.height);
        let gpu = match &self.renderer {
            Some(r) => r.render(base).ok().map(|img| img.rgba),
            None => None,
        };
        let rgba = match gpu {
            Some(rgba) => rgba,
            None => {
                if self.renderer.is_some() {
                    self.status = "render GPU fallito: uso la CPU".into();
                }
                base.composite()
            }
        };
        self.needs_recompose = false;
        (rgba, w, h)
    }

    // -----------------------------------------------------------------
    // History
    // -----------------------------------------------------------------

    pub fn undo(&mut self) -> bool {
        // un drag in corso ha modificato il documento fuori dalla storia:
        // va chiuso prima, o l'undo annulla qualcos'altro lasciando il drag
        // con un valore di partenza che non esiste piu'
        self.end_opacity_drag();
        let fatto = self.doc.undo();
        if fatto {
            self.needs_recompose = true;
            self.status = "annullato".into();
        }
        fatto
    }

    pub fn redo(&mut self) -> bool {
        self.end_opacity_drag();
        let fatto = self.doc.redo();
        if fatto {
            self.needs_recompose = true;
            self.status = "ripetuto".into();
        }
        fatto
    }

    pub fn can_undo(&self) -> bool {
        self.doc.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.doc.history.can_redo()
    }

    pub fn history_len(&self) -> usize {
        self.doc.history.len()
    }

    // -----------------------------------------------------------------
    // Opacità: il drag che non deve inondare la history
    // -----------------------------------------------------------------

    /// Inizio del trascinamento di uno slider di opacità.
    pub fn begin_opacity_drag(&mut self, layer: LayerId) {
        // una volta sola per drag: 1/2 lato = 1/4 dei pixel
        if self.preview.is_none() {
            self.preview = Some((scala_documento(&self.doc, 0.5), 0.5));
        }
        if let Some(l) = self.doc.layer(layer) {
            self.drag_opacita = Some(OpacityDrag {
                layer,
                inizio: l.opacity,
                corrente: l.opacity,
            });
        }
    }

    /// Ogni movimento dello slider aggiorna il documento **senza history**.
    pub fn update_opacity_drag(&mut self, value: f32) {
        if let Some(drag) = &mut self.drag_opacita {
            drag.corrente = value.clamp(0.0, 1.0);
            if let Some(l) = self.doc.layer_mut(drag.layer) {
                l.opacity = drag.corrente;
                self.needs_recompose = true;
            }
        }
    }

    /// Fine del trascinamento: **un solo** comando in history, se qualcosa è cambiato.
    pub fn end_opacity_drag(&mut self) -> bool {
        let Some(drag) = self.drag_opacita.take() else {
            return false;
        };
        self.preview = None;
        if (drag.corrente - drag.inizio).abs() < 1e-4 {
            return false;
        }
        // riporta il documento al valore iniziale, poi registra il comando:
        // così l'undo ripristina esattamente lo stato di partenza
        if let Some(l) = self.doc.layer_mut(drag.layer) {
            l.opacity = drag.inizio;
        }
        let mut cmd = pizzart_core::history::SetOpacity::new(drag.layer, drag.corrente);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.needs_recompose = true;
        true
    }

    pub fn is_dragging_opacity(&self) -> bool {
        self.drag_opacita.is_some()
    }

    // -----------------------------------------------------------------
    // Layer
    // -----------------------------------------------------------------

    /// Cambia strumento chiudendo l'eventuale tratto in corso.
    ///
    /// Chiudere qui, e non in inish_stroke chiamato dal rilascio del mouse,
    /// e' cio' che evita i punti orfani: il rilascio puo' arrivare quando lo
    /// strumento e' gia' cambiato.
    pub fn change_tool(&mut self, tool: Tool) {
        if tool != Tool::Brush {
            self.finish_stroke();
        }
        self.tool = tool;
        self.cancel_selection();
    }

    pub fn selected_layer(&self) -> Option<&Layer> {
        self.selected.and_then(|id| self.doc.layer(id))
    }

    pub fn set_visible(&mut self, layer: LayerId, visible: bool) {
        let mut cmd = pizzart_core::history::SetVisible::new(layer, visible);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.needs_recompose = true;
    }

    pub fn set_blend(&mut self, layer: LayerId, blend: BlendMode) {
        let mut cmd = pizzart_core::history::SetBlendMode::new(layer, blend);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.needs_recompose = true;
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let mut cmd = pizzart_core::history::RemoveLayer::new(id);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.selected = None;
        self.needs_recompose = true;
        self.status = "layer eliminato".into();
        true
    }

    pub fn move_selected(&mut self, su: bool) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(indice) = self.doc.index_of(id) else {
            return false;
        };
        let Some(contenitore) = self.doc.parent_of(id) else {
            return false;
        };
        let quanti = self
            .doc
            .children_of(contenitore)
            .map(|c| c.len())
            .unwrap_or(0);
        if (su && indice + 1 >= quanti) || (!su && indice == 0) {
            return false;
        }
        let destinazione = if su { indice + 1 } else { indice - 1 };
        // il contenitore del nodo: la radice per i layer di primo livello, il
        // gruppo per i suoi figli. Cercare sempre nella radice era il BUG-005.
        let Some(altro) = self
            .doc
            .children_of(contenitore)
            .and_then(|c| c.get(destinazione))
        else {
            return false;
        };
        let target = altro.id();
        let dove = if su {
            pizzart_core::InsertWhere::After
        } else {
            pizzart_core::InsertWhere::Before
        };
        let esito = self.doc.reorder(id, target, dove).is_ok();
        if esito {
            self.needs_recompose = true;
        }
        esito
    }

    pub fn merge_down(&mut self) -> Result<(), String> {
        let Some(id) = self.selected else {
            return Err("nessun layer selezionato".into());
        };
        self.doc.merge_down(id).map_err(|e| e.to_string())?;
        self.selected = self.doc.root.last().map(|n| n.id());
        self.needs_recompose = true;
        self.status = "fusi due layer".into();
        Ok(())
    }

    // -----------------------------------------------------------------
    // Strumenti
    // -----------------------------------------------------------------

    /// Aggiunge un punto allo stroke in corso.
    pub fn push_point(&mut self, p: Point) {
        self.stroke_points.push(p);
    }

    /// Aggiunge un layer passando dalla history: l'undo lo toglie davvero.
    fn aggiungi_con_storia(&mut self, layer: Layer) -> LayerId {
        let id = layer.id;
        let mut cmd = pizzart_core::history::AddLayer::new(layer);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.selected = Some(id);
        self.needs_recompose = true;
        id
    }

    /// Conclude lo stroke: rasterizza e crea un layer.
    pub fn finish_stroke(&mut self) -> Option<LayerId> {
        if self.stroke_points.is_empty() {
            return None;
        }
        let mut stroke = Stroke::new(self.brush);
        for p in self.stroke_points.drain(..) {
            stroke.add(p);
        }
        let tile = stroke.render_bbox(self.doc.width, self.doc.height);
        if tile.is_empty() {
            return None;
        }
        let mut layer = Layer::new("tratto", tile.width, tile.height, tile.rgba);
        layer.transform = Transform::at(tile.offset_x as f32, tile.offset_y as f32);
        let id = self.aggiungi_con_storia(layer);
        self.status = "tratto aggiunto".into();
        Some(id)
    }

    /// Rasterizza un testo e lo aggiunge come layer.
    pub fn add_text_layer(&mut self, testo: &Text) -> Option<LayerId> {
        let tile = testo.render(&mut self.registry);
        if tile.is_empty() {
            return None;
        }
        let mut layer = Layer::new("testo", tile.width, tile.height, tile.rgba);
        layer.transform = Transform::at(tile.offset_x as f32, tile.offset_y as f32);
        let id = self.aggiungi_con_storia(layer);
        self.status = "testo aggiunto".into();
        Some(id)
    }

    /// Comincia una selezione rettangolare (coordinate del documento).
    pub fn begin_selection(&mut self, x: f32, y: f32) {
        self.selection = Some((x, y, 0.0, 0.0));
    }

    /// Aggiorna la selezione mentre si trascina.
    pub fn update_selection(&mut self, x: f32, y: f32) {
        if let Some((x0, y0, _, _)) = self.selection {
            self.selection = Some((x0, y0, x - x0, y - y0));
        }
    }

    pub fn cancel_selection(&mut self) {
        self.selection = None;
    }

    /// Applica la selezione come maschera del layer selezionato.
    pub fn apply_selection_as_mask(&mut self) -> Result<(), String> {
        let Some((x, y, w, h)) = self.selection else {
            return Err("nessuna selezione".into());
        };
        let Some(id) = self.selected else {
            return Err("nessun layer selezionato".into());
        };
        if w.abs() < 1.0 || h.abs() < 1.0 {
            return Err("selezione troppo piccola".into());
        }
        let mask = RectSelection { x, y, w, h }.to_mask(self.doc.width, self.doc.height);
        let mut cmd = pizzart_core::history::SetLayerMask::new(id, Some(std::sync::Arc::new(mask)));
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.selection = None;
        self.needs_recompose = true;
        self.status = "maschera applicata".into();
        Ok(())
    }

    /// Applica una maschera ellittica al layer selezionato (menu, non tool).
    pub fn apply_ellipse_mask(&mut self) -> Result<(), String> {
        let Some(id) = self.selected else {
            return Err("nessun layer selezionato".into());
        };
        let (w, h) = (self.doc.width as f32, self.doc.height as f32);
        let mask = EllipseSelection::circle(w / 2.0, h / 2.0, w.min(h) * 0.4)
            .to_mask(self.doc.width, self.doc.height);
        let mut cmd = pizzart_core::history::SetLayerMask::new(id, Some(std::sync::Arc::new(mask)));
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.needs_recompose = true;
        Ok(())
    }

    /// Toglie la maschera del layer selezionato.
    pub fn clear_mask(&mut self) -> Result<(), String> {
        let Some(id) = self.selected else {
            return Err("nessun layer selezionato".into());
        };
        let mut cmd = pizzart_core::history::SetLayerMask::new(id, None);
        cmd.apply(&mut self.doc);
        self.doc.history.push(Box::new(cmd));
        self.needs_recompose = true;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Effetti
    // -----------------------------------------------------------------

    /// Applica un effetto al layer selezionato, come nuovo layer sopra.
    ///
    /// Un effetto distruttivo sul layer esisterebbe, ma perderemmo l'originale:
    /// così l'undo è naturale (si toglie il layer) e l'effetto resta mascherato
    /// dal fatto che è un layer come gli altri.
    pub fn apply_effect(&mut self, effect: Effect) -> Result<LayerId, String> {
        let Some(id) = self.selected else {
            return Err("nessun layer selezionato".into());
        };
        let Some(renderer) = &self.renderer else {
            return Err("nessuna GPU: effetti non disponibili".into());
        };
        let Some(sorgente) = self.doc.layer(id).cloned() else {
            return Err("layer non trovato".into());
        };
        let tile = pizzart_core::PlacedImage {
            rgba: sorgente.pixels.clone(),
            width: sorgente.width,
            height: sorgente.height,
            offset_x: 0,
            offset_y: 0,
            canvas_w: sorgente.width,
            canvas_h: sorgente.height,
        };
        let uscita = renderer
            .apply_effects(&tile, &[effect])
            .map_err(|e| format!("effetto: {e:?}"))?;
        let mut layer = Layer::new(
            format!("effetto su {}", sorgente.name),
            uscita.width,
            uscita.height,
            uscita.rgba,
        );
        layer.transform = sorgente.transform;
        let nuovo = self.aggiungi_con_storia(layer);
        self.status = "effetto applicato".into();
        Ok(nuovo)
    }

    // -----------------------------------------------------------------
    // Selezione con il mouse
    // -----------------------------------------------------------------

    /// Layer sotto un punto del documento, scorrendo dall'alto in basso.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<LayerId> {
        for layer in self.doc.layers().iter().rev() {
            if !layer.visible {
                continue;
            }
            let Some(inverso) = layer.transform.doc_to_layer() else {
                continue;
            };
            let (lx, ly) = inverso.map_point(x, y);
            if lx < 0.0 || ly < 0.0 {
                continue;
            }
            let (ix, iy) = (lx as usize, ly as usize);
            if ix >= layer.width as usize || iy >= layer.height as usize {
                continue;
            }
            let i = (iy * layer.width as usize + ix) * 4;
            if layer.pixels[i + 3] > 8 {
                return Some(layer.id);
            }
        }
        None
    }
}

/// Copia il documento a scala ridotta (campionamento nearest).
///
/// Serve solo per la preview durante il drag: i pixel non sono definitivi.
pub fn scala_documento(doc: &Document, fattore: f32) -> Document {
    let w = ((doc.width as f32 * fattore).round() as u32).max(1);
    let h = ((doc.height as f32 * fattore).round() as u32).max(1);
    let mut scalato = Document::new(w, h);
    for nodo in &doc.root {
        if let Node::Layer(l) = nodo {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                let sy = (y as f32 / fattore) as u32;
                for x in 0..w {
                    let sx = (x as f32 / fattore) as u32;
                    let si = ((sy.min(l.height.saturating_sub(1)) * l.width
                        + sx.min(l.width.saturating_sub(1)))
                        * 4) as usize;
                    if si + 4 <= l.pixels.len() {
                        rgba.extend_from_slice(&l.pixels[si..si + 4]);
                    } else {
                        rgba.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
            let mut copia = Layer::new(l.name.clone(), w, h, rgba);
            copia.transform = Transform {
                x: l.transform.x * fattore,
                y: l.transform.y * fattore,
                scale_x: l.transform.scale_x * fattore,
                scale_y: l.transform.scale_y * fattore,
                rotation: l.transform.rotation,
            };
            copia.opacity = l.opacity;
            copia.blend = l.blend;
            copia.visible = l.visible;
            scalato.add_layer(copia);
        }
    }
    scalato
}

/// Converte un colore del core in un array RGBA.
pub fn rgba(colore: [u8; 4]) -> [u8; 4] {
    colore
}

/// Rettangolo normalizzato (utile al canvas per disegnare la selezione).
pub fn normalizza(x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let x0 = x.min(x + w);
    let y0 = y.min(y + h);
    (x0, y0, w.abs(), h.abs())
}

/// Nodo radice che contiene il layer (per messaggi e pannelli).
pub fn radice_di(doc: &Document, id: LayerId) -> Option<usize> {
    doc.children_of(None)?.iter().position(|n| n.id() == id)
}

/// `true` se il nodo è un gruppo.
pub fn e_gruppo(nodo: &Node) -> bool {
    nodo.as_group().is_some()
}

/// Maschera vuota per i test.
pub fn maschera_piena(w: u32, h: u32) -> Mask {
    Mask::full(w, h)
}
