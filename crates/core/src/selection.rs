//! Selezione: tre strumenti, un solo tipo di risultato — una `Mask`.
//!
//! La rasterizzazione è volutamente **una sola**: ogni forma fornisce una
//! funzione "questo punto è dentro?", e il campionamento con antialiasing è
//! condiviso. Così rettangolo, ellisse e lazo si comportano allo stesso modo ai
//! bordi, e le forme (che verranno dopo) riuseranno la stessa funzione
//! cambiando solo il tipo di ritorno.
//!
//! ## Convenzioni
//!
//! - **Coordinate in pixel.** `RectSelection { x: 10, y: 10, w: 100, h: 100 }`
//!   copre i pixel `10..110` (bordo destro escluso), come un rettangolo di
//!   schermo.
//! - **Dimensione della maschera = dimensione del documento**, sempre. Una
//!   selezione in un angolo produce comunque una maschera piena con il resto a 0.
//! - **Bordi antialiasati**, non sfocati: il campionamento è 4×4 per pixel, e
//!   il valore risultante è la frazione di area coperta.
//! - **Lazo con regola even-odd**: un punto che attraversa il contorno un numero
//!   dispari di volte è dentro. È la regola più prevedibile per una selezione
//!   disegnata a mano.

use crate::mask::{Mask, MaskSource};

/// Campioni per asse: 4×4 = 16 campioni per pixel.
const SAMPLES: u32 = 4;

/// Rasterizza una forma generica in un canale alpha delle dimensioni richieste.
///
/// `inside` decide se un punto (in coordinate di documento) appartiene alla forma.
/// Il risultato è la frazione di campioni interni, quindi i bordi sono morbidi.
pub fn rasterize_alpha<F>(doc_w: u32, doc_h: u32, inside: F) -> Vec<u8>
where
    F: Fn(f32, f32) -> bool,
{
    let mut alpha = vec![0u8; doc_w as usize * doc_h as usize];
    if doc_w == 0 || doc_h == 0 {
        return alpha;
    }
    let step = 1.0 / SAMPLES as f32;
    let total = SAMPLES * SAMPLES;
    for y in 0..doc_h {
        for x in 0..doc_w {
            let mut hits = 0u32;
            for sy in 0..SAMPLES {
                let py = y as f32 + (sy as f32 + 0.5) * step;
                for sx in 0..SAMPLES {
                    let px = x as f32 + (sx as f32 + 0.5) * step;
                    if inside(px, py) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                alpha[(y as usize * doc_w as usize) + x as usize] = ((hits * 255) / total) as u8;
            }
        }
    }
    alpha
}

/// Rettangolo di selezione trascinato sul documento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectSelection {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectSelection {
    /// Accetta qualsiasi direzione di trascinamento: `w` e `h` negativi sono
    /// normalizzati con min/max, come ci si aspetta da un mouse.
    pub fn normalized(&self) -> (f32, f32, f32, f32) {
        let x0 = self.x.min(self.x + self.w);
        let y0 = self.y.min(self.y + self.h);
        let x1 = self.x.max(self.x + self.w);
        let y1 = self.y.max(self.y + self.h);
        (x0, y0, x1, y1)
    }

    pub fn to_mask(&self, doc_w: u32, doc_h: u32) -> Mask {
        let (x0, y0, x1, y1) = self.normalized();
        let alpha = rasterize_alpha(doc_w, doc_h, |px, py| {
            px >= x0 && px < x1 && py >= y0 && py < y1
        });
        Mask::new(alpha, doc_w, doc_h, MaskSource::Selection)
    }
}

/// Ellisse inscritta in un rettangolo (lo stesso del drag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipseSelection {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl EllipseSelection {
    /// Cerchio al centro di raggio dato: scorciatoia utile per le UI.
    pub fn circle(cx: f32, cy: f32, radius: f32) -> Self {
        Self {
            x: cx - radius,
            y: cy - radius,
            w: radius * 2.0,
            h: radius * 2.0,
        }
    }

    pub fn to_mask(&self, doc_w: u32, doc_h: u32) -> Mask {
        let (x0, y0, x1, y1) = {
            let x0 = self.x.min(self.x + self.w);
            let y0 = self.y.min(self.y + self.h);
            (
                x0,
                y0,
                self.x.max(self.x + self.w),
                self.y.max(self.y + self.h),
            )
        };
        let (w, h) = (x1 - x0, y1 - y0);
        if w <= 0.0 || h <= 0.0 {
            return Mask::new(
                vec![0u8; doc_w as usize * doc_h as usize],
                doc_w,
                doc_h,
                MaskSource::Selection,
            );
        }
        let (cx, cy) = (x0 + w * 0.5, y0 + h * 0.5);
        let (rx, ry) = (w * 0.5, h * 0.5);
        let alpha = rasterize_alpha(doc_w, doc_h, |px, py| {
            let nx = (px - cx) / rx;
            let ny = (py - cy) / ry;
            nx * nx + ny * ny <= 1.0
        });
        Mask::new(alpha, doc_w, doc_h, MaskSource::Selection)
    }
}

/// Lazo: poligono chiuso, punti in ordine di disegno.
#[derive(Clone, Debug, PartialEq)]
pub struct LassoSelection {
    pub points: Vec<(f32, f32)>,
}

impl LassoSelection {
    pub fn new(points: Vec<(f32, f32)>) -> Self {
        Self { points }
    }

    /// Il poligono è sempre chiuso: l'ultimo punto si collega al primo.
    pub fn to_mask(&self, doc_w: u32, doc_h: u32) -> Mask {
        if self.points.len() < 3 {
            return Mask::new(
                vec![0u8; doc_w as usize * doc_h as usize],
                doc_w,
                doc_h,
                MaskSource::Selection,
            );
        }
        let alpha = rasterize_alpha(doc_w, doc_h, |px, py| {
            point_in_polygon(&self.points, px, py)
        });
        Mask::new(alpha, doc_w, doc_h, MaskSource::Selection)
    }
}

/// Regola even-odd: si conta quante volte un raggio orizzontale attraversa il
/// contorno. Dispari = dentro. Vale anche per i poligoni concavi.
pub fn point_in_polygon(points: &[(f32, f32)], x: f32, y: f32) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        // il semiasse orizzontale verso +x interseca il lato?
        if (yi > y) != (yj > y) {
            let t = (y - yi) / (yj - yi);
            let x_cross = xi + t * (xj - xi);
            if x < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(mask: &Mask, x: u32, y: u32) -> u8 {
        mask.alpha[(y as usize * mask.width as usize) + x as usize]
    }

    #[test]
    fn rect_drag_in_direzione_opposta_normalizza() {
        let (w, h) = (200u32, 200u32);
        let avanti = RectSelection {
            x: 50.0,
            y: 50.0,
            w: 50.0,
            h: 50.0,
        }
        .to_mask(w, h);
        let indietro = RectSelection {
            x: 100.0,
            y: 100.0,
            w: -50.0,
            h: -50.0,
        }
        .to_mask(w, h);
        assert_eq!(
            avanti, indietro,
            "il trascinamento opposto deve dare la stessa maschera"
        );
        assert_eq!(alpha_at(&indietro, 60, 60), 255);
        assert_eq!(alpha_at(&indietro, 40, 40), 0);
    }

    #[test]
    fn rect_fuori_tela_viene_clippato() {
        let mask = RectSelection {
            x: -10.0,
            y: -10.0,
            w: 50.0,
            h: 50.0,
        }
        .to_mask(100, 100);
        assert_eq!(alpha_at(&mask, 0, 0), 255, "l'angolo visibile è dentro");
        assert_eq!(alpha_at(&mask, 39, 39), 255);
        assert_eq!(alpha_at(&mask, 41, 41), 0, "oltre il bordo del rettangolo");
        assert_eq!(mask.width, 100);
        assert_eq!(mask.height, 100);
    }

    #[test]
    fn rect_anti_aliasing() {
        let mask = RectSelection {
            x: 10.5,
            y: 10.5,
            w: 99.0,
            h: 99.0,
        }
        .to_mask(200, 200);
        // bordo verticale sinistro: il pixel 10 è coperto a metà (in x)
        let bordo = alpha_at(&mask, 10, 20);
        assert!(
            (110..=145).contains(&bordo),
            "il pixel sul bordo verticale deve essere intermedio: {bordo}"
        );
        // l'angolo è coperto in x e in y: circa un quarto
        let angolo = alpha_at(&mask, 10, 10);
        assert!(
            (45..=80).contains(&angolo),
            "l'angolo in alto a sinistra è coperto per un quarto: {angolo}"
        );
        // dentro, netto
        assert_eq!(alpha_at(&mask, 12, 12), 255);
        assert_eq!(alpha_at(&mask, 9, 20), 0);
    }

    #[test]
    fn ellisse_degenere_e_vuota() {
        let larga = EllipseSelection {
            x: 10.0,
            y: 10.0,
            w: 0.0,
            h: 50.0,
        }
        .to_mask(100, 100);
        let alta = EllipseSelection {
            x: 10.0,
            y: 10.0,
            w: 50.0,
            h: 0.0,
        }
        .to_mask(100, 100);
        assert!(larga.alpha.iter().all(|a| *a == 0));
        assert!(alta.alpha.iter().all(|a| *a == 0));
        assert_eq!(
            larga.width, 100,
            "anche vuota, la maschera ha la tela intera"
        );
    }

    #[test]
    fn ellisse_cerchio_al_centro() {
        // raggio 50 con centro intero: il bordo cade **esattamente** sulla
        // griglia dei pixel, quindi il pixel 150 è interamente fuori.
        let mask = EllipseSelection::circle(100.0, 100.0, 50.0).to_mask(200, 200);
        assert_eq!(alpha_at(&mask, 100, 100), 255, "centro");
        assert_eq!(alpha_at(&mask, 149, 100), 255, "dentro, vicino al bordo");
        assert_eq!(
            alpha_at(&mask, 150, 100),
            0,
            "il bordo è escluso, come il rettangolo"
        );
        assert_eq!(alpha_at(&mask, 151, 100), 0, "fuori");

        // con un centro a metà pixel il bordo attraversa il pixel 150:
        // metà copertura, quindi antialiasing visibile.
        let sfalsato = EllipseSelection::circle(100.5, 100.0, 50.0).to_mask(200, 200);
        let bordo = alpha_at(&sfalsato, 150, 100);
        assert!(
            (110..=145).contains(&bordo),
            "il bordo a metà pixel deve dare ~128: {bordo}"
        );
    }

    #[test]
    fn lazo_meno_di_tre_punti_e_vuoto() {
        let due = LassoSelection::new(vec![(10.0, 10.0), (20.0, 20.0)]).to_mask(100, 100);
        assert!(due.alpha.iter().all(|a| *a == 0));
        let uno = LassoSelection::new(vec![(10.0, 10.0)]).to_mask(100, 100);
        assert!(uno.alpha.iter().all(|a| *a == 0));
    }

    #[test]
    fn lazo_triangolo() {
        let mask = LassoSelection::new(vec![(50.0, 50.0), (150.0, 50.0), (100.0, 150.0)])
            .to_mask(200, 200);
        assert_eq!(alpha_at(&mask, 100, 70), 255, "dentro il triangolo");
        assert_eq!(alpha_at(&mask, 10, 10), 0, "fuori");
        assert_eq!(alpha_at(&mask, 100, 160), 0, "sotto la punta");
    }

    #[test]
    fn lazo_poligono_concavo() {
        // una "C": aperta a destra, quindi il centro della C è vuoto
        let punti = vec![
            (40.0, 20.0),
            (160.0, 20.0),
            (160.0, 50.0),
            (70.0, 50.0),
            (70.0, 150.0),
            (160.0, 150.0),
            (160.0, 180.0),
            (40.0, 180.0),
        ];
        let mask = LassoSelection::new(punti).to_mask(200, 200);
        assert_eq!(
            alpha_at(&mask, 55, 100),
            255,
            "il braccio sinistro è dentro"
        );
        assert_eq!(
            alpha_at(&mask, 55, 35),
            255,
            "il braccio superiore è dentro"
        );
        assert_eq!(alpha_at(&mask, 120, 100), 0, "il centro della C è fuori");
        assert_eq!(alpha_at(&mask, 170, 100), 0, "oltre il braccio");
    }

    #[test]
    fn tutte_le_selezioni_hanno_la_dimensione_del_documento() {
        let rect = RectSelection {
            x: 2.0,
            y: 3.0,
            w: 10.0,
            h: 12.0,
        }
        .to_mask(200, 150);
        let ell = EllipseSelection {
            x: 2.0,
            y: 3.0,
            w: 10.0,
            h: 12.0,
        }
        .to_mask(200, 150);
        let lazo =
            LassoSelection::new(vec![(2.0, 3.0), (12.0, 3.0), (7.0, 15.0)]).to_mask(200, 150);
        for (nome, mask) in [("rect", rect), ("ellisse", ell), ("lazo", lazo)] {
            assert_eq!(mask.width, 200, "{nome}: larghezza");
            assert_eq!(mask.height, 150, "{nome}: altezza");
            assert_eq!(mask.alpha.len(), 200 * 150, "{nome}: byte");
            assert_eq!(mask.source, MaskSource::Selection, "{nome}: sorgente");
        }
    }

    #[test]
    fn le_coordinate_sono_in_pixel_e_il_bordo_destro_e_escluso() {
        let mask = RectSelection {
            x: 10.0,
            y: 10.0,
            w: 5.0,
            h: 5.0,
        }
        .to_mask(40, 40);
        assert_eq!(alpha_at(&mask, 10, 10), 255, "primo pixel incluso");
        assert_eq!(alpha_at(&mask, 14, 14), 255, "ultimo pixel incluso");
        assert_eq!(alpha_at(&mask, 15, 14), 0, "il bordo destro è escluso");
        assert_eq!(alpha_at(&mask, 14, 15), 0, "il bordo inferiore è escluso");
    }

    #[test]
    fn il_punto_in_poligono_concorda_con_il_campionamento() {
        let quadrato = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon(&quadrato, 5.0, 5.0));
        assert!(!point_in_polygon(&quadrato, 15.0, 5.0));
        assert!(!point_in_polygon(&quadrato, -1.0, 5.0));
        assert!(!point_in_polygon(&quadrato, 5.0, 11.0));
    }
}
