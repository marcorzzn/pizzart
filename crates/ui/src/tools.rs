//! Strumenti: il minimo per trasformare eventi del mouse in azioni.

use pizzart_draw::Point;

use crate::state::Tool;

/// I comandi offerti dalla barra degli strumenti.
pub const STRUMENTI: [(Tool, &str, &str); 4] = [
    (Tool::Select, "Seleziona", "S"),
    (Tool::Brush, "Pennello", "B"),
    (Tool::Text, "Testo", "T"),
    (Tool::Rect, "Selezione", "R"),
];

/// Carattere di scelta rapida: lo strumento corrispondente, se c'è.
pub fn da_scorciatoia(c: char) -> Option<Tool> {
    match c.to_ascii_lowercase() {
        's' => Some(Tool::Select),
        'b' => Some(Tool::Brush),
        't' => Some(Tool::Text),
        'r' => Some(Tool::Rect),
        _ => None,
    }
}

/// Punto del pennello con pressione costante (il mouse non ha pressione).
pub fn punto_mouse(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// Descrizione breve dello strumento, per la barra di stato.
pub fn descrizione(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "clicca un layer per selezionarlo",
        Tool::Brush => "trascina per disegnare",
        Tool::Text => "clicca dove mettere il testo",
        Tool::Rect => "trascina per selezionare, Invio applica la maschera",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_scorciatoie_corrispondono_agli_strumenti() {
        for (tool, _, tasto) in STRUMENTI {
            let c = tasto.chars().next().expect("tasto");
            assert_eq!(da_scorciatoia(c), Some(tool), "tasto {tasto}");
        }
        assert_eq!(da_scorciatoia('x'), None);
    }
}
