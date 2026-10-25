//! L'applicazione: menù, canvas, pannelli. Il guscio attorno a [`Editor`].

use std::path::PathBuf;

use egui::{Color32, RichText};
use pizzart_core::Transform;
use pizzart_text::{Align, Text, TextStyle};

use crate::canvas;
use crate::dialogs::{self, EsitoTesto, TextDialog};
use crate::panels::{self, PanelloStato};
use crate::state::{Editor, Tool};
use crate::tools;

/// Stato del canvas (texture e interazione).
pub struct PizzartApp {
    pub editor: Editor,
    pub texture: Option<egui::TextureHandle>,
    pub pannello: PanelloStato,
    pub dialogo_testo: TextDialog,
    pub errore: Option<String>,
    /// Ultimo punto del drag del pennello, per non riempire il tratto di punti doppi.
    ultimo_punto: Option<(f32, f32)>,
    pub mostra_pannello_layer: bool,
    pub mostra_pannello_effetti: bool,
}

impl PizzartApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            editor: Editor::new(1280, 800),
            texture: None,
            pannello: PanelloStato::default(),
            dialogo_testo: TextDialog::default(),
            errore: None,
            ultimo_punto: None,
            mostra_pannello_layer: true,
            mostra_pannello_effetti: true,
        }
    }

    fn apri(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("immagini e progetti", &["png", "jpg", "jpeg", "pizzart"])
            .add_filter("progetto pizzart", &["pizzart"])
            .add_filter("immagini", &["png", "jpg", "jpeg"])
            .pick_file()
        {
            // un progetto si carica subito (manifest + asset gia' compressi),
            // un'immagine va decodificata: quella va su un thread
            let progetto = path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("pizzart"))
                .unwrap_or(false);
            if progetto {
                if let Err(e) = self.editor.open(&path) {
                    self.errore = Some(e);
                }
            } else {
                self.editor.open_image_async(path);
            }
        }
    }

    fn salva(&mut self) {
        match self.editor.save() {
            Ok(true) => {}
            Ok(false) => self.salva_con_nome(),
            Err(e) => self.errore = Some(e),
        }
    }

    fn salva_con_nome(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("progetto pizzart", &["pizzart"])
            .set_file_name("progetto.pizzart")
            .save_file()
        {
            let path = con_estensione(path, "pizzart");
            if let Err(e) = self.editor.save_project(&path) {
                self.errore = Some(e);
            }
        }
    }

    fn esporta_png(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("export.png")
            .save_file()
        {
            let path = con_estensione(path, "png");
            if let Err(e) = self.editor.export_png(&path) {
                self.errore = Some(e);
            }
        }
    }

    fn menu(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Apri…").clicked() {
                        ui.close_menu();
                        self.apri();
                    }
                    if ui.button("Salva").clicked() {
                        ui.close_menu();
                        self.salva();
                    }
                    if ui.button("Salva con nome…").clicked() {
                        ui.close_menu();
                        self.salva_con_nome();
                    }
                    ui.separator();
                    if ui.button("Esporta PNG…").clicked() {
                        ui.close_menu();
                        self.esporta_png();
                    }
                    ui.separator();
                    if ui.button("Nuovo documento").clicked() {
                        ui.close_menu();
                        let (w, h) = (self.editor.doc.width, self.editor.doc.height);
                        self.editor = Editor::new(w, h);
                        self.texture = None;
                    }
                });
                ui.menu_button("Modifica", |ui| {
                    if ui
                        .add_enabled(self.editor.can_undo(), egui::Button::new("Annulla  (Ctrl+Z)"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.editor.undo();
                    }
                    if ui
                        .add_enabled(self.editor.can_redo(), egui::Button::new("Ripeti  (Ctrl+Y)"))
                        .clicked()
                    {
                        ui.close_menu();
                        self.editor.redo();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.editor.selected.is_some(),
                            egui::Button::new("Elimina layer"),
                        )
                        .clicked()
                    {
                        ui.close_menu();
                        self.editor.delete_selected();
                    }
                });
                ui.menu_button("Vista", |ui| {
                    if ui.button("Adatta").clicked() {
                        ui.close_menu();
                        self.editor.zoom = 1.0;
                        self.editor.pan = (0.0, 0.0);
                    }
                    ui.checkbox(&mut self.mostra_pannello_layer, "Pannello layer");
                    ui.checkbox(&mut self.mostra_pannello_effetti, "Pannello effetti");
                });

                ui.separator();
                for (tool, nome, tasto) in tools::STRUMENTI {
                    let attivo = self.editor.tool == tool;
                    if ui
                        .selectable_label(attivo, format!("{nome} ({tasto})"))
                        .clicked()
                    {
                        self.editor.change_tool(tool);
                    }
                }
                ui.separator();
                ui.label(RichText::new(tools::descrizione(self.editor.tool)).small().weak());
            });
        });
    }

    fn scorciatoie(&mut self, ctx: &egui::Context) {
        let (
            ctrl_z,
            ctrl_y,
            ctrl_shift_z,
            ctrl_s,
            ctrl_shift_s,
            ctrl_o,
            ctrl_e,
            ctrl_0,
            ctrl_piu,
            ctrl_meno,
            invio,
            esc,
            carattere,
        ) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            (
                i.key_pressed(egui::Key::Z) && cmd && !shift,
                i.key_pressed(egui::Key::Y) && cmd,
                i.key_pressed(egui::Key::Z) && cmd && shift,
                i.key_pressed(egui::Key::S) && cmd && !shift,
                i.key_pressed(egui::Key::S) && cmd && shift,
                i.key_pressed(egui::Key::O) && cmd,
                i.key_pressed(egui::Key::E) && cmd,
                i.key_pressed(egui::Key::Num0) && cmd,
                i.key_pressed(egui::Key::Equals) && cmd,
                i.key_pressed(egui::Key::Minus) && cmd,
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                i.events.iter().find_map(|e| match e {
                    egui::Event::Text(t) if !cmd => t.chars().next(),
                    _ => None,
                }),
            )
        });
        if ctrl_z {
            self.editor.undo();
        }
        if ctrl_y || ctrl_shift_z {
            self.editor.redo();
        }
        if ctrl_s {
            self.salva();
        }
        if ctrl_shift_s {
            self.salva_con_nome();
        }
        if ctrl_o {
            self.apri();
        }
        if ctrl_e {
            self.esporta_png();
        }
        if ctrl_0 {
            self.editor.zoom = 1.0;
            self.editor.pan = (0.0, 0.0);
        }
        if ctrl_piu {
            self.editor.zoom = (self.editor.zoom * 1.25).clamp(0.05, 8.0);
        }
        if ctrl_meno {
            self.editor.zoom = (self.editor.zoom / 1.25).clamp(0.05, 8.0);
        }
        if esc {
            self.editor.cancel_selection();
        }
        if invio && self.editor.tool == Tool::Rect {
            if let Err(e) = self.editor.apply_selection_as_mask() {
                self.errore = Some(e);
            }
        }
        // scorciatoie degli strumenti, solo se non si sta scrivendo in un campo
        if !ctx.wants_keyboard_input() {
            if let Some(c) = carattere {
                if let Some(tool) = tools::da_scorciatoia(c) {
                    self.editor.change_tool(tool);
                }
            }
        }
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.editor.needs_recompose || self.texture.is_none() {
                canvas::aggiorna_texture(&mut self.editor, ctx, &mut self.texture);
            }

            let area = ui.available_rect_before_wrap();
            let risposta = ui.allocate_rect(area, egui::Sense::click_and_drag());
            let (doc_w, doc_h) = (self.editor.doc.width, self.editor.doc.height);
            let rect = canvas::rect_documento(
                area,
                doc_w,
                doc_h,
                self.editor.zoom,
                egui::vec2(self.editor.pan.0, self.editor.pan.1),
            );

            let painter = ui.painter();
            painter.rect_filled(area, 0.0, Color32::from_rgb(28, 30, 34));
            if let Some(texture) = &self.texture {
                painter.image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            // zoom con la rotella, centrato sul cursore
            if risposta.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll.abs() > 0.1 {
                    let fattore = (scroll * 0.0015).exp();
                    let nuovo = (self.editor.zoom * fattore).clamp(0.05, 8.0);
                    if let Some(cursore) = risposta.hover_pos() {
                        let (zoom, pan) = canvas::imposta_zoom(
                            area,
                            doc_w,
                            doc_h,
                            self.editor.pan,
                            self.editor.zoom,
                            nuovo,
                            cursore,
                        );
                        self.editor.zoom = zoom;
                        self.editor.pan = pan;
                    }
                }
            }

            // pan: tasto centrale, oppure spazio + sinistro
            let spazio = ui.input(|i| i.key_down(egui::Key::Space));
            if risposta.dragged_by(egui::PointerButton::Middle) || (spazio && risposta.dragged()) {
                let delta = risposta.drag_delta();
                self.editor.pan.0 += delta.x;
                self.editor.pan.1 += delta.y;
            } else if risposta.dragged_by(egui::PointerButton::Primary) {
                self.drag_primario(&risposta, rect, area);
            }

            if risposta.drag_stopped() {
                self.fine_drag();
            }
            if risposta.clicked() {
                self.click(&risposta, rect, doc_w, doc_h);
            }

            // overlay della selezione in corso
            if let Some((x, y, w, h)) = self.editor.selection {
                let (x0, y0, w, h) = crate::state::normalizza(x, y, w, h);
                let colore = canvas::SELEZIONE_IN_CORSO;
                let min = rect.min
                    + egui::vec2(x0 / doc_w as f32 * rect.width(), y0 / doc_h as f32 * rect.height());
                let size = egui::vec2(
                    w / doc_w as f32 * rect.width(),
                    h / doc_h as f32 * rect.height(),
                );
                painter.rect_stroke(
                    egui::Rect::from_min_size(min, size),
                    0.0,
                    egui::Stroke::new(1.5_f32, colore),
                );
            }
        });
    }

    /// Click: selezione del layer, oppure posa del testo.
    fn click(&mut self, risposta: &egui::Response, rect: egui::Rect, doc_w: u32, doc_h: u32) {
        let Some(pos) = risposta.interact_pointer_pos() else {
            return;
        };
        let Some((x, y)) = canvas::schermo_a_documento(pos, rect, doc_w, doc_h) else {
            return;
        };
        match self.editor.tool {
            Tool::Select => {
                self.editor.selected = self.editor.hit_test(x, y);
            }
            Tool::Text => {
                self.dialogo_testo.apri(x, y);
            }
            Tool::Rect => {
                self.editor.begin_selection(x, y);
            }
            Tool::Brush => {}
        }
    }

    /// Trascinamento con il tasto sinistro: pennello o selezione.
    fn drag_primario(&mut self, risposta: &egui::Response, rect: egui::Rect, area: egui::Rect) {
        let _ = area;
        let (doc_w, doc_h) = (self.editor.doc.width, self.editor.doc.height);
        let Some(pos) = risposta.interact_pointer_pos() else {
            return;
        };
        let Some((x, y)) = canvas::schermo_a_documento(pos, rect, doc_w, doc_h) else {
            return;
        };
        match self.editor.tool {
            Tool::Brush => {
                // un punto ogni tanto: il tratto è già denso di suo
                if let Some((ux, uy)) = self.ultimo_punto {
                    if (x - ux).hypot(y - uy) < 2.0 {
                        return;
                    }
                }
                self.ultimo_punto = Some((x, y));
                self.editor.push_point(tools::punto_mouse(x, y));
                self.editor.status = format!("disegno… {} punti", self.editor.stroke_points.len());
            }
            Tool::Rect => {
                self.editor.update_selection(x, y);
            }
            _ => {}
        }
    }

    fn fine_drag(&mut self) {
        self.ultimo_punto = None;
        // il tratto si chiude se ci sono punti, **indipendentemente** dallo
        // strumento: se l'utente ha cambiato tool a meta' gesto, i punti
        // sarebbero rimasti orfani e si sarebbero attaccati al tratto dopo
        if !self.editor.stroke_points.is_empty() {
            self.editor.finish_stroke();
        }
        if self.editor.tool != Tool::Rect {
            return;
        }
        // una selezione minima non è una selezione
        if let Some((_, _, w, h)) = self.editor.selection {
            if w.abs() < 2.0 || h.abs() < 2.0 {
                self.editor.cancel_selection();
            } else {
                self.editor.status = "premi Invio per applicare la maschera".into();
            }
        }
    }

    fn dialogo_testo(&mut self, ctx: &egui::Context) {
        let esito = dialogs::dialogo_testo(ctx, &mut self.dialogo_testo);
        if let Some(EsitoTesto::Conferma {
            testo,
            dimensione,
            colore,
        }) = esito
        {
            let famiglia = self
                .editor
                .registry
                .system_families()
                .first()
                .cloned()
                .unwrap_or_else(|| "sans-serif".into());
            let (x, y) = self.dialogo_testo.posizione;
            let t = Text::new(
                testo,
                TextStyle::new(dimensione)
                    .family(famiglia)
                    .color(colore)
                    .align(Align::Left),
            )
            .at(x, y);
            let _: Option<Transform> = None;
            self.editor.add_text_layer(&t);
        }
    }
}

impl eframe::App for PizzartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // consegna della decodifica in corso, se e' pronta
        if let Some(esito) = self.editor.poll_loading() {
            if let Err(e) = esito {
                self.errore = Some(e);
            }
        }
        self.menu(ctx);
        self.scorciatoie(ctx);

        egui::SidePanel::right("pannelli")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    panels::pannello_strumento(ui, &mut self.editor);
                    ui.separator();
                    if self.mostra_pannello_layer {
                        panels::pannello_layer(ui, &mut self.editor, &mut self.errore);
                    }
                    if self.mostra_pannello_effetti {
                        ui.separator();
                        panels::pannello_effetti(
                            ui,
                            &mut self.editor,
                            &mut self.pannello,
                            &mut self.errore,
                        );
                    }
                });
            });

        egui::TopBottomPanel::bottom("stato").show(ctx, |ui| {
            panels::barra_stato(ui, &self.editor);
        });

        self.canvas(ctx);
        self.dialogo_testo(ctx);
        dialogs::dialogo_errore(ctx, &mut self.errore);

        // le modifiche fatte dai pannelli devono arrivare al canvas
        if self.editor.needs_recompose {
            canvas::aggiorna_texture(&mut self.editor, ctx, &mut self.texture);
        }

        if self.editor.is_loading() {
            egui::Window::new("Apertura")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("decodifica in corso...");
                    });
                });
            ctx.request_repaint();
        }
    }
}

/// Aggiunge l'estensione se manca.
fn con_estensione(path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false) {
        path
    } else {
        let mut p = path.clone();
        p.set_extension(ext);
        p
    }
}
