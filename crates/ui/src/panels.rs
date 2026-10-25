//! Pannelli: layer, effetti, barra di stato.

use egui::{Color32, RichText};

use pizzart_core::BlendMode;
use pizzart_render::Effect;

use crate::state::Editor;

/// Effetti disponibili nella UI, con i parametri che il pannello modifica.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effetto {
    Luminosita,
    Contrasto,
    Saturazione,
    Sfocatura,
    Nitidezza,
    Bagliore,
}

impl Effetto {
    pub const TUTTI: [Effetto; 6] = [
        Effetto::Luminosita,
        Effetto::Contrasto,
        Effetto::Saturazione,
        Effetto::Sfocatura,
        Effetto::Nitidezza,
        Effetto::Bagliore,
    ];

    pub fn nome(self) -> &'static str {
        match self {
            Effetto::Luminosita => "Luminosità",
            Effetto::Contrasto => "Contrasto",
            Effetto::Saturazione => "Saturazione",
            Effetto::Sfocatura => "Sfocatura",
            Effetto::Nitidezza => "Nitidezza",
            Effetto::Bagliore => "Bagliore",
        }
    }

    /// Costruisce l'effetto del motore con il parametro principale.
    pub fn costruisci(self, valore: f32, raggio: f32) -> Effect {
        match self {
            Effetto::Luminosita => Effect::brightness(valore),
            Effetto::Contrasto => Effect::contrast(valore),
            Effetto::Saturazione => Effect::saturate(valore),
            Effetto::Sfocatura => Effect::blur(raggio),
            Effetto::Nitidezza => Effect::sharpen(valore, raggio),
            Effetto::Bagliore => Effect::glow(0.25, valore, raggio),
        }
    }

    /// Intervallo e default del parametro principale.
    pub fn intervallo(self) -> (f32, f32, f32) {
        match self {
            Effetto::Luminosita => (-1.0, 1.0, 0.2),
            Effetto::Contrasto => (0.0, 3.0, 1.2),
            Effetto::Saturazione => (-1.0, 2.0, 1.4),
            Effetto::Sfocatura => (0.0, 40.0, 6.0),
            Effetto::Nitidezza => (0.0, 2.0, 1.0),
            Effetto::Bagliore => (0.0, 4.0, 1.5),
        }
    }
}

/// Modelli di fusione mostrati nel pannello layer.
pub const BLEND: [(BlendMode, &str); 5] = [
    (BlendMode::Normal, "Normale"),
    (BlendMode::Multiply, "Moltiplica"),
    (BlendMode::Screen, "Scherma"),
    (BlendMode::Overlay, "Sovrapponi"),
    (BlendMode::SoftLight, "Luce soffusa"),
];

/// Tutti i modi di fusione, con l'etichetta italiana.
pub const BLEND_TUTTI: [(BlendMode, &str); 12] = [
    (BlendMode::Normal, "Normale"),
    (BlendMode::Multiply, "Moltiplica"),
    (BlendMode::Screen, "Scherma"),
    (BlendMode::Overlay, "Sovrapponi"),
    (BlendMode::Darken, "Scurisci"),
    (BlendMode::Lighten, "Schiarisci"),
    (BlendMode::ColorDodge, "Scola colore"),
    (BlendMode::ColorBurn, "Brucia colore"),
    (BlendMode::HardLight, "Luce dura"),
    (BlendMode::SoftLight, "Luce soffusa"),
    (BlendMode::Difference, "Differenza"),
    (BlendMode::Exclusion, "Esclusione"),
];

/// Etichetta breve di un modo di fusione.
pub fn nome_blend(mode: BlendMode) -> &'static str {
    BLEND_TUTTI
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, n)| *n)
        .unwrap_or("Normale")
}

/// Blocco dello strumento: i parametri del pennello (BUG-001 del report).
///
/// Senza questi controlli il pennello disegna solo con un colore e una
/// dimensione fissi, il che rende inutilizzabile lo strumento principale.
pub fn pannello_strumento(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.heading("Strumento");
    if editor.tool != crate::state::Tool::Brush {
        ui.label(
            RichText::new("il pennello e' attivo con lo strumento Pennello (B)")
                .small()
                .weak(),
        );
        return;
    }
    ui.separator();

    let mut colore = Color32::from_rgb(
        editor.brush.color[0],
        editor.brush.color[1],
        editor.brush.color[2],
    );
    ui.horizontal(|ui| {
        ui.label("colore");
        if ui.color_edit_button_srgba(&mut colore).changed() {
            editor.brush.color = [colore.r(), colore.g(), colore.b()];
        }
    });
    ui.add(egui::Slider::new(&mut editor.brush.size, 1.0..=200.0).text("dimensione"));
    ui.add(egui::Slider::new(&mut editor.brush.opacity, 0.0..=1.0).text("opacita"));
    ui.add(egui::Slider::new(&mut editor.brush.hardness, 0.0..=1.0).text("durezza"));
    ui.add(egui::Slider::new(&mut editor.brush.stabilize, 0.0..=100.0).text("stabilizza"));

    let mut blend = editor.brush.blend;
    egui::ComboBox::from_label("fusione")
        .selected_text(nome_blend(blend))
        .show_ui(ui, |ui| {
            for (m, nome) in BLEND_TUTTI {
                if ui.selectable_label(blend == m, nome).clicked() {
                    blend = m;
                }
            }
        });
    editor.brush.blend = blend;
}

/// Stato dei pannelli (persistente fra i frame).
pub struct PanelloStato {
    pub effetto: Effetto,
    pub valore: f32,
    pub raggio: f32,
}

impl Default for PanelloStato {
    fn default() -> Self {
        Self {
            effetto: Effetto::Luminosita,
            valore: 0.2,
            raggio: 6.0,
        }
    }
}

/// Pannello dei layer.
pub fn pannello_layer(ui: &mut egui::Ui, editor: &mut Editor, errore: &mut Option<String>) {
    ui.heading("Layer");
    ui.label(
        RichText::new(format!(
            "{} layer · {} gruppi",
            editor.doc.layer_count(),
            editor
                .doc
                .root
                .iter()
                .filter(|n| n.as_group().is_some())
                .count()
        ))
        .small()
        .weak(),
    );
    ui.separator();

    // dall'alto in basso: è l'ordine che l'utente si aspetta in un pannello
    let ids: Vec<(pizzart_core::LayerId, String, bool, f32, BlendMode, bool)> = editor
        .doc
        .layers()
        .iter()
        .rev()
        .map(|l| {
            (
                l.id,
                l.name.clone(),
                l.visible,
                l.opacity,
                l.blend,
                l.mask.is_some(),
            )
        })
        .collect();

    for (id, nome, visibile, opacita, blend, ha_maschera) in ids {
        let selezionato = editor.selected == Some(id);
        ui.horizontal(|ui| {
            let mut v = visibile;
            if ui.checkbox(&mut v, "").changed() {
                editor.set_visible(id, v);
            }
            let etichetta = if ha_maschera {
                format!("{nome} ◐")
            } else {
                nome.clone()
            };
            if ui
                .selectable_label(selezionato, RichText::new(etichetta).strong())
                .clicked()
            {
                editor.selected = Some(id);
            }
        });

        if selezionato {
            // Lo slider di opacità: un comando di history **al rilascio**.
            let mut valore = opacita;
            let risposta = ui.add(
                egui::Slider::new(&mut valore, 0.0..=1.0)
                    .text("opacità")
                    .fixed_decimals(2),
            );
            if risposta.drag_started() {
                editor.begin_opacity_drag(id);
            }
            if risposta.dragged() || risposta.changed() {
                editor.update_opacity_drag(valore);
            }
            if risposta.drag_stopped() {
                editor.end_opacity_drag();
            }

            let mut modo = blend;
            egui::ComboBox::from_id_salt(("blend", id.raw()))
                .selected_text(
                    BLEND
                        .iter()
                        .find(|(m, _)| *m == modo)
                        .map(|(_, n)| *n)
                        .unwrap_or("Normale"),
                )
                .show_ui(ui, |ui| {
                    for (m, nome) in BLEND {
                        if ui.selectable_label(modo == m, nome).clicked() {
                            modo = m;
                        }
                    }
                });
            if modo != blend {
                editor.set_blend(id, modo);
            }

            ui.horizontal(|ui| {
                if ui.button("↑").on_hover_text("sposta sopra").clicked() {
                    editor.move_selected(true);
                }
                if ui.button("↓").on_hover_text("sposta sotto").clicked() {
                    editor.move_selected(false);
                }
                if ui.button("Fondi").on_hover_text("fondi con il layer sotto").clicked() {
                    if let Err(e) = editor.merge_down() {
                        *errore = Some(e);
                    }
                }
                if ui.button("Elimina").clicked() {
                    editor.delete_selected();
                }
            });
            ui.separator();
        }
    }

    if editor.doc.layer_count() == 0 {
        ui.label(RichText::new("nessun layer: apri un'immagine o disegna").weak());
    }
}

/// Pannello effetti.
pub fn pannello_effetti(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    stato: &mut PanelloStato,
    errore: &mut Option<String>,
) {
    ui.heading("Effetti");
    ui.label(RichText::new("applica al layer selezionato").small().weak());
    ui.separator();

    let (min, max, default) = stato.effetto.intervallo();
    egui::ComboBox::from_label("effetto")
        .selected_text(stato.effetto.nome())
        .show_ui(ui, |ui| {
            for e in Effetto::TUTTI {
                if ui
                    .selectable_label(stato.effetto == e, e.nome())
                    .clicked()
                {
                    stato.effetto = e;
                    let (_, _, d) = e.intervallo();
                    stato.valore = d;
                }
            }
        });
    let _ = default;

    ui.add(egui::Slider::new(&mut stato.valore, min..=max).text("valore"));
    ui.add(egui::Slider::new(&mut stato.raggio, 0.0..=40.0).text("raggio"));

    let abilitato = editor.selected.is_some() && editor.renderer.is_some();
    if ui
        .add_enabled(abilitato, egui::Button::new("Applica"))
        .clicked()
    {
        let effetto = stato.effetto.costruisci(stato.valore, stato.raggio);
        if let Err(e) = editor.apply_effect(effetto) {
            *errore = Some(e);
        }
    }
    if editor.renderer.is_none() {
        ui.label(RichText::new("nessuna GPU: effetti non disponibili").weak());
    }

    ui.separator();
    ui.label(RichText::new("maschere").small().strong());
    if ui
        .add_enabled(editor.selected.is_some(), egui::Button::new("Maschera ellittica"))
        .clicked()
    {
        if let Err(e) = editor.apply_ellipse_mask() {
            *errore = Some(e);
        }
    }
    if ui
        .add_enabled(editor.selected.is_some(), egui::Button::new("Togli maschera"))
        .clicked()
    {
        if let Err(e) = editor.clear_mask() {
            *errore = Some(e);
        }
    }
}

/// Barra di stato in basso.
pub fn barra_stato(ui: &mut egui::Ui, editor: &Editor) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{}×{}  ·  zoom {:.0}%  ·  {}  ·  {}",
                editor.doc.width,
                editor.doc.height,
                editor.zoom * 100.0,
                match editor.selected_layer() {
                    Some(l) => format!("layer: {}", l.name),
                    None => "nessun layer selezionato".into(),
                },
                editor.gpu(),
            ))
            .small(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(&editor.status).small().color(Color32::from_rgb(160, 200, 160)));
        });
    });
}
