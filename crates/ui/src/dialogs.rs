//! Dialoghi: testo nuovo e messaggi di errore.

use egui::{Color32, RichText};

/// Stato del dialogo "aggiungi testo".
pub struct TextDialog {
    pub aperto: bool,
    pub contenuto: String,
    pub dimensione: f32,
    pub colore: [u8; 4],
    /// Posizione del click sul canvas, in pixel del documento.
    pub posizione: (f32, f32),
}

impl Default for TextDialog {
    fn default() -> Self {
        Self {
            aperto: false,
            contenuto: "Pizzart".into(),
            dimensione: 64.0,
            colore: [255, 255, 255, 255],
            posizione: (0.0, 0.0),
        }
    }
}

impl TextDialog {
    pub fn apri(&mut self, x: f32, y: f32) {
        self.aperto = true;
        self.posizione = (x, y);
    }
}

/// Esito del dialogo del testo.
pub enum EsitoTesto {
    Annulla,
    Conferma {
        testo: String,
        dimensione: f32,
        colore: [u8; 4],
    },
}

/// Disegna il dialogo. `None` se è ancora aperto.
pub fn dialogo_testo(ctx: &egui::Context, dialogo: &mut TextDialog) -> Option<EsitoTesto> {
    if !dialogo.aperto {
        return None;
    }
    let mut esito = None;
    let mut aperto = true;
    egui::Window::new("Nuovo testo")
        .open(&mut aperto)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Contenuto");
            ui.text_edit_singleline(&mut dialogo.contenuto);
            ui.add(egui::Slider::new(&mut dialogo.dimensione, 8.0..=300.0).text("dimensione"));
            ui.horizontal(|ui| {
                ui.label("colore");
                let mut c = Color32::from_rgba_unmultiplied(
                    dialogo.colore[0],
                    dialogo.colore[1],
                    dialogo.colore[2],
                    dialogo.colore[3],
                );
                if ui.color_edit_button_srgba(&mut c).changed() {
                    dialogo.colore = [c.r(), c.g(), c.b(), c.a()];
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    esito = Some(EsitoTesto::Conferma {
                        testo: dialogo.contenuto.clone(),
                        dimensione: dialogo.dimensione,
                        colore: dialogo.colore,
                    });
                }
                if ui.button("Annulla").clicked() {
                    esito = Some(EsitoTesto::Annulla);
                }
            });
        });
    if !aperto && esito.is_none() {
        esito = Some(EsitoTesto::Annulla);
    }
    if esito.is_some() {
        dialogo.aperto = false;
    }
    esito
}

/// Mostra un errore in una finestra modale finché non viene chiusa.
pub fn dialogo_errore(ctx: &egui::Context, messaggio: &mut Option<String>) {
    let Some(testo) = messaggio.clone() else {
        return;
    };
    egui::Window::new("Errore")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new(testo).color(Color32::from_rgb(240, 120, 100)));
            ui.separator();
            if ui.button("Chiudi").clicked() {
                *messaggio = None;
            }
        });
}

/// Chiede conferma per un'azione distruttiva.
pub fn conferma(ctx: &egui::Context, domanda: &str) -> Option<bool> {
    let mut esito = None;
    egui::Window::new("Conferma")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(domanda);
            ui.horizontal(|ui| {
                if ui.button("Sì").clicked() {
                    esito = Some(true);
                }
                if ui.button("No").clicked() {
                    esito = Some(false);
                }
            });
        });
    esito
}
