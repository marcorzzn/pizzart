//! # pizzart-draw
//!
//! Il motore di disegno, interamente su CPU.
//!
//! ## Pipeline
//!
//! ```text
//! punti del tablet  ->  stabilizzazione  ->  spline Catmull-Rom
//!                   ->  ricampionamento per lunghezza d'arco (spacing)
//!                   ->  timbri (stampini) con pressione e tilt
//!                   ->  maschera di copertura
//!                   ->  buffer RGBA8 dello stroke
//!                   ->  (opzionale) composizione sul fondo con il blend mode del pennello
//! ```
//!
//! Ogni fase è una funzione pura e testabile: si può verificare la spaziatura
//! senza rasterizzare, e la copertura senza comporre.
//!
//! ## Scelte dichiarate
//!
//! - **La copertura si accumula con `max`, non con somma.** Con spaziatura fitta
//!   i timbri si sovrappongono: sommare l'alpha farebbe scurire i tratti nei
//!   punti di giunzione. Il massimo dà un tratto uniforme, e l'opacità resta
//!   quella scelta dall'utente.
//! - **Due campi in più rispetto all'API minima**: `color` (un pennello senza
//!   colore non può disegnare, e senza colore non si può verificare il blend) e
//!   `stabilize` (la specifica chiede il parametro di stabilizzazione ma non
//!   dice dove metterlo: sta nel pennello, perché è una proprietà dello
//!   strumento, non del singolo tratto).
//! - **Nessuna texture**: le spazzole sono procedurali (distanza con bordo
//!   sfumato, ellisse per il tilt). Niente PNG di punte.

use pizzart_core::BlendMode;

/// Buffer di un tratto ritagliato al suo ingombro e posizionato nella tela.
///
/// È un alias del tipo di `pizzart-core`: la stessa struttura serve agli
/// effetti e alle selezioni, e tenerla nel core evita che il renderer debba
/// dipendere dal motore di disegno.
pub type StrokeBuffer = pizzart_core::PlacedImage;

/// Frazione del raggio usata a pressione 0: un tratto sottile ma visibile.
pub const MIN_RADIUS_SCALE: f32 = 0.15;

/// Un campione del tablet o del mouse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    /// 0.0 = nessuna pressione, 1.0 = massima.
    pub pressure: f32,
    /// Inclinazione in gradi sull'asse X (-90..90).
    pub tilt_x: f32,
    /// Inclinazione in gradi sull'asse Y (-90..90).
    pub tilt_y: f32,
    /// Istante di campionamento, in millisecondi.
    pub time_ms: f64,
}

impl Point {
    /// Punto con pressione piena, senza tilt.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            pressure: 1.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
            time_ms: 0.0,
        }
    }

    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.clamp(0.0, 1.0);
        self
    }

    pub fn with_tilt(mut self, tilt_x: f32, tilt_y: f32) -> Self {
        self.tilt_x = tilt_x;
        self.tilt_y = tilt_y;
        self
    }

    pub fn with_time(mut self, time_ms: f64) -> Self {
        self.time_ms = time_ms;
        self
    }
}

/// Parametri del pennello.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    /// Diametro in pixel a pressione massima.
    pub size: f32,
    /// 0.0 = bordo completamente sfumato, 1.0 = bordo netto.
    pub hardness: f32,
    /// Opacità del tratto (0..1).
    pub opacity: f32,
    /// Distanza fra i timbri, in frazione del **diametro** (convenzione standard
    /// dei pennelli: 1.0 = timbri affiancati).
    ///
    /// Geometricamente, con timbri rotondi: fino a ~0.9 il tratto è continuo, a
    /// 1.0 i timbri si toccano appena, e **solo sopra 1.0** compaiono spazi
    /// (puntinato). Il valore non è limitato a 1.
    pub spacing: f32,
    /// Modo di fusione usato quando il tratto viene composto sul fondo.
    pub blend: BlendMode,
    /// Stabilizzazione 0..100: 0 = segue la mano, 100 = molto liscia.
    pub stabilize: f32,
    /// Colore del tratto, RGB.
    pub color: [u8; 3],
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            size: 20.0,
            hardness: 0.8,
            opacity: 1.0,
            spacing: 0.15,
            blend: BlendMode::Normal,
            stabilize: 0.0,
            color: [20, 20, 24],
        }
    }
}

impl Brush {
    pub fn new(size: f32) -> Self {
        Self {
            size: size.max(0.5),
            ..Self::default()
        }
    }

    pub fn color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }

    pub fn hardness(mut self, hardness: f32) -> Self {
        self.hardness = hardness.clamp(0.0, 1.0);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.01);
        self
    }

    pub fn blend(mut self, blend: BlendMode) -> Self {
        self.blend = blend;
        self
    }

    pub fn stabilize(mut self, stabilize: f32) -> Self {
        self.stabilize = stabilize.clamp(0.0, 100.0);
        self
    }

    /// Raggio a una data pressione: lineare fra il 15% e il 100% del raggio.
    pub fn radius_for(&self, pressure: f32) -> f32 {
        let p = pressure.clamp(0.0, 1.0);
        let scale = MIN_RADIUS_SCALE + (1.0 - MIN_RADIUS_SCALE) * p;
        (self.size * 0.5) * scale
    }

    /// Passo fra due timbri, in pixel.
    pub fn step_px(&self) -> f32 {
        (self.size * self.spacing).max(0.5)
    }
}

/// Un timbro: un'orma del pennello lungo il percorso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamp {
    pub x: f32,
    pub y: f32,
    /// Raggio lungo l'asse principale.
    pub radius: f32,
    /// Rapporto fra asse secondario e principale (1.0 = cerchio).
    pub ratio: f32,
    /// Orientamento dell'ellisse, in radianti.
    pub angle: f32,
    pub pressure: f32,
}

impl Stamp {
    /// Distanza normalizzata dal centro: 0 = centro, 1 = bordo.
    fn normalised_distance(&self, px: f32, py: f32) -> f32 {
        let dx = px - self.x;
        let dy = py - self.y;
        let (sin, cos) = self.angle.sin_cos();
        // Ruota nel sistema dell'ellisse.
        let u = cos * dx + sin * dy;
        let v = -sin * dx + cos * dy;
        let rx = self.radius.max(1e-4);
        let ry = (self.radius * self.ratio).max(1e-4);
        ((u / rx) * (u / rx) + (v / ry) * (v / ry)).sqrt()
    }
}

/// Un tratto: un pennello e i punti raccolti.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub brush: Brush,
    pub points: Vec<Point>,
}

impl Stroke {
    pub fn new(brush: Brush) -> Self {
        Self {
            brush,
            points: Vec::new(),
        }
    }

    /// Aggiunge un punto e restituisce `self` per concatenare le chiamate.
    pub fn add(&mut self, point: Point) -> &mut Self {
        self.points.push(point);
        self
    }

    pub fn with_points(brush: Brush, points: Vec<Point>) -> Self {
        Self { brush, points }
    }

    /// I timbri che compongono il tratto: percorso liscio, ricampionato per
    /// lunghezza d'arco con il passo del pennello.
    pub fn stamps(&self) -> Vec<Stamp> {
        let path = self.path();
        if path.is_empty() {
            return Vec::new();
        }
        if path.len() == 1 {
            return vec![self.brush.stamp_at(path[0])];
        }

        let step = self.brush.step_px();
        let mut out = Vec::new();
        let mut carry = 0.0f32;
        let mut prev = path[0];
        out.push(self.brush.stamp_at(prev));

        for &next in &path[1..] {
            let dx = next.x - prev.x;
            let dy = next.y - prev.y;
            let seg = (dx * dx + dy * dy).sqrt();
            if seg <= f32::EPSILON {
                prev = next;
                continue;
            }
            let mut travelled = step - carry;
            while travelled <= seg {
                let t = travelled / seg;
                let p = Point {
                    x: prev.x + dx * t,
                    y: prev.y + dy * t,
                    pressure: prev.pressure + (next.pressure - prev.pressure) * t,
                    tilt_x: prev.tilt_x + (next.tilt_x - prev.tilt_x) * t,
                    tilt_y: prev.tilt_y + (next.tilt_y - prev.tilt_y) * t,
                    time_ms: prev.time_ms + (next.time_ms - prev.time_ms) * t as f64,
                };
                out.push(self.brush.stamp_at(p));
                travelled += step;
            }
            carry = seg - (travelled - step);
            prev = next;
        }
        out
    }

    /// Percorso liscio: stabilizzazione, poi spline Catmull-Rom.
    fn path(&self) -> Vec<Point> {
        let smoothed = stabilise(&self.points, self.brush.stabilize);
        if smoothed.len() <= 2 {
            return smoothed;
        }
        catmull_rom(&smoothed, (self.brush.step_px() * 0.5).max(0.5))
    }

    /// Rasterizza il tratto **solo nella regione che occupa**.
    ///
    /// È l'API da usare: su una tela 4000×4000 un tratto di 50 pixel alloca
    /// qualche decina di KB invece di 64 MB, e lo stesso vale per l'upload in
    /// GPU e per la composizione.
    ///
    /// Il buffer prodotto contiene solo il tratto (fondo trasparente); l'alpha
    /// incorpora l'opacità del pennello. Se il tratto è interamente fuori dalla
    /// tela, il risultato è una regione vuota (`width == 0`).
    pub fn render_bbox(&self, canvas_w: u32, canvas_h: u32) -> StrokeBuffer {
        if canvas_w == 0 || canvas_h == 0 || self.points.is_empty() {
            return StrokeBuffer::empty(canvas_w, canvas_h);
        }
        let opacity = self.brush.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return StrokeBuffer::empty(canvas_w, canvas_h);
        }

        let stamps = self.stamps();
        let Some((x0, y0, x1, y1)) = bounds(&stamps, canvas_w, canvas_h) else {
            return StrokeBuffer::empty(canvas_w, canvas_h);
        };
        let (w, h) = (x1 - x0, y1 - y0);

        let mut mask = vec![0f32; w as usize * h as usize];
        self.rasterise_stamps(&stamps, &mut mask, w, h, x0 as i32, y0 as i32);

        let mut rgba = vec![0u8; mask.len() * 4];
        for (i, cov) in mask.iter().enumerate() {
            if *cov <= 0.0 {
                continue;
            }
            let alpha = (cov.min(1.0) * opacity * 255.0).round() as u8;
            if alpha == 0 {
                continue;
            }
            let at = i * 4;
            rgba[at] = self.brush.color[0];
            rgba[at + 1] = self.brush.color[1];
            rgba[at + 2] = self.brush.color[2];
            rgba[at + 3] = alpha;
        }

        StrokeBuffer {
            rgba,
            width: w,
            height: h,
            offset_x: x0 as i32,
            offset_y: y0 as i32,
            canvas_w,
            canvas_h,
        }
    }

    /// Buffer pieno della tela, trasparente fuori dal tratto.
    ///
    /// Resta per compatibilità e per i test: internamente produce il ritaglio e
    /// lo spalma, quindi costa quanto la tela. Chi può, usi `render_bbox`.
    pub fn render(&self, canvas_w: u32, canvas_h: u32) -> Vec<u8> {
        let mut out = vec![0u8; canvas_w as usize * canvas_h as usize * 4];
        self.render_bbox(canvas_w, canvas_h)
            .blit_into(&mut out, canvas_w, canvas_h);
        out
    }

    /// Compone il tratto sopra un fondo esistente, usando il blend mode del pennello.
    ///
    /// Restituisce quanti pixel sono stati toccati.
    pub fn render_over(&self, dst: &mut [u8], canvas_w: u32, canvas_h: u32) -> usize {
        let src = self.render(canvas_w, canvas_h);
        let mut painted = 0;
        for i in (0..dst.len().min(src.len())).step_by(4) {
            if src[i + 3] == 0 {
                continue;
            }
            pizzart_core::blend::blend_px(
                &mut dst[i..i + 4],
                &src[i..i + 4],
                self.brush.blend,
                1.0,
            );
            painted += 1;
        }
        painted
    }

    fn rasterise_stamps(
        &self,
        stamps: &[Stamp],
        mask: &mut [f32],
        local_w: u32,
        local_h: u32,
        origin_x: i32,
        origin_y: i32,
    ) {
        let hardness = self.brush.hardness.clamp(0.0, 1.0);
        for stamp in stamps {
            // L'ingombro di un timbro è il raggio (il tilt schiaccia l'ellisse,
            // non la allarga), più un pixel di margine per l'antialiasing.
            let extent = stamp.radius + 1.0;
            let ax0 = (stamp.x - extent).floor().max(origin_x as f32) as i32;
            let ay0 = (stamp.y - extent).floor().max(origin_y as f32) as i32;
            let ax1 = ((stamp.x + extent).ceil() as i32).min(origin_x + local_w as i32);
            let ay1 = ((stamp.y + extent).ceil() as i32).min(origin_y + local_h as i32);
            if ax1 <= ax0 || ay1 <= ay0 {
                continue;
            }

            for ay in ay0..ay1 {
                let row = (ay - origin_y) as usize * local_w as usize;
                for ax in ax0..ax1 {
                    // Coordinate **assolute**: la copertura non dipende dal ritaglio.
                    let d = stamp.normalised_distance(ax as f32 + 0.5, ay as f32 + 0.5);
                    let cover = coverage(d, hardness);
                    if cover <= 0.0 {
                        continue;
                    }
                    let slot = &mut mask[row + (ax - origin_x) as usize];
                    if cover > *slot {
                        *slot = cover;
                    }
                }
            }
        }
    }
}

/// Ingombro di un insieme di timbri, intersecato con la tela.
///
/// Restituisce `(x0, y0, x1, y1)` in coordinate assolute, oppure `None` se i
/// timbri non toccano la tela.
fn bounds(stamps: &[Stamp], canvas_w: u32, canvas_h: u32) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for s in stamps {
        let e = s.radius + 1.0;
        min_x = min_x.min(s.x - e);
        min_y = min_y.min(s.y - e);
        max_x = max_x.max(s.x + e);
        max_y = max_y.max(s.y + e);
    }
    if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
        return None;
    }
    let x0 = min_x.floor().clamp(0.0, canvas_w as f32) as u32;
    let y0 = min_y.floor().clamp(0.0, canvas_h as f32) as u32;
    let x1 = max_x.ceil().clamp(0.0, canvas_w as f32) as u32;
    let y1 = max_y.ceil().clamp(0.0, canvas_h as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some((x0, y0, x1, y1))
    }
}

impl Brush {
    /// Timbro corrispondente a un punto del percorso.
    fn stamp_at(&self, p: Point) -> Stamp {
        let radius = self.radius_for(p.pressure);
        // Il tilt inclina la punta: l'ellisse si schiaccia fino al 40%.
        let tilt_mag = (p.tilt_x * p.tilt_x + p.tilt_y * p.tilt_y).sqrt();
        let normalised = (tilt_mag / 90.0).clamp(0.0, 1.0);
        let ratio = 1.0 - 0.4 * normalised;
        let angle = if tilt_mag > 1e-3 {
            p.tilt_y.atan2(p.tilt_x)
        } else {
            0.0
        };
        Stamp {
            x: p.x,
            y: p.y,
            radius,
            ratio,
            angle,
            pressure: p.pressure,
        }
    }
}

/// Copertura in funzione della distanza normalizzata.
///
/// `hardness = 1` → gradino netto; `hardness = 0` → sfumatura lineare dal centro.
pub fn coverage(distance: f32, hardness: f32) -> f32 {
    if distance >= 1.0 || distance.is_nan() {
        return 0.0;
    }
    if distance <= 0.0 {
        return 1.0;
    }
    let h = hardness.clamp(0.0, 1.0);
    if h >= 0.999 {
        return 1.0;
    }
    let inner = h;
    if distance <= inner {
        1.0
    } else {
        ((1.0 - distance) / (1.0 - inner)).clamp(0.0, 1.0)
    }
}

/// Stabilizzazione: filtro esponenziale sui punti, per togliere il tremolio.
///
/// `strength` 0 = nessun effetto, 100 = inseguimento molto lento (tratto liscio
/// ma in ritardo). Il primo punto resta fermo, così il tratto parte da dove
/// l'utente ha appoggiato la punta.
pub fn stabilise(points: &[Point], strength: f32) -> Vec<Point> {
    let k = (strength.clamp(0.0, 100.0) / 100.0) * 0.95;
    if points.is_empty() || k <= f32::EPSILON {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len());
    let mut sx = points[0].x;
    let mut sy = points[0].y;
    out.push(points[0]);
    for p in &points[1..] {
        sx += (1.0 - k) * (p.x - sx);
        sy += (1.0 - k) * (p.y - sy);
        out.push(Point { x: sx, y: sy, ..*p });
    }
    out
}

/// Spline Catmull-Rom passante per i punti di controllo, campionata a `step` pixel.
pub fn catmull_rom(points: &[Point], step: f32) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let step = step.max(0.25);
    let mut out = vec![points[0]];
    for i in 0..points.len() - 1 {
        let p0 = points[i.saturating_sub(1)];
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = points[(i + 2).min(points.len() - 1)];

        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        let chord = (dx * dx + dy * dy).sqrt();
        let samples = (chord / step).ceil().max(1.0) as usize;

        for s in 1..=samples {
            let t = s as f32 / samples as f32;
            let (x, y) = catmull_rom_point(p0, p1, p2, p3, t);
            out.push(Point {
                x,
                y,
                pressure: p1.pressure + (p2.pressure - p1.pressure) * t,
                tilt_x: p1.tilt_x + (p2.tilt_x - p1.tilt_x) * t,
                tilt_y: p1.tilt_y + (p2.tilt_y - p1.tilt_y) * t,
                time_ms: p1.time_ms + (p2.time_ms - p1.time_ms) * t as f64,
            });
        }
    }
    out
}

fn catmull_rom_point(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> (f32, f32) {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * ((2.0 * b)
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    (f(p0.x, p1.x, p2.x, p3.x), f(p0.y, p1.y, p2.y, p3.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn canvas(w: u32, h: u32) -> Vec<u8> {
        vec![255u8; (w * h * 4) as usize]
    }

    fn alpha_at(buf: &[u8], w: u32, x: u32, y: u32) -> u8 {
        buf[((y * w + x) * 4 + 3) as usize]
    }

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    fn coperti(buf: &[u8]) -> usize {
        buf.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn uno_stroke_senza_punti_produce_un_buffer_vuoto() {
        let stroke = Stroke::new(Brush::default());
        let out = stroke.render(32, 32);
        assert_eq!(out.len(), 32 * 32 * 4);
        assert!(
            out.iter().all(|b| *b == 0),
            "nessun pixel deve essere scritto"
        );
        assert!(stroke.stamps().is_empty());
    }

    #[test]
    fn un_punto_produce_un_cerchio_del_diametro_del_pennello() {
        let mut stroke = Stroke::new(Brush::new(20.0).hardness(1.0));
        stroke.add(Point::new(32.0, 32.0));
        let out = stroke.render(64, 64);

        // area attesa: pi greco * r^2, con r = 10
        let attesa = std::f32::consts::PI * 100.0;
        let area = coperti(&out) as f32;
        assert!(
            (area - attesa).abs() < attesa * 0.15,
            "area {area} lontana da {attesa}"
        );
        assert!(alpha_at(&out, 64, 32, 23) > 0, "dentro il cerchio");
        assert_eq!(alpha_at(&out, 64, 32, 21), 0, "fuori dal cerchio");
        assert_eq!(alpha_at(&out, 64, 0, 0), 0, "angolo lontano");
    }

    #[test]
    fn due_punti_producono_una_linea_interpolata() {
        let mut stroke = Stroke::new(Brush::new(6.0).hardness(1.0));
        stroke.add(Point::new(10.0, 32.0));
        stroke.add(Point::new(54.0, 32.0));
        let out = stroke.render(64, 64);
        for x in [10u32, 20, 32, 44, 54] {
            assert!(alpha_at(&out, 64, x, 32) > 0, "pixel {x} non coperto");
        }
        assert_eq!(alpha_at(&out, 64, 5, 32), 0, "prima dell'inizio");
    }

    #[test]
    fn la_spline_catmull_rom_passa_per_i_punti_di_controllo() {
        let punti = vec![
            Point::new(10.0, 50.0),
            Point::new(30.0, 20.0),
            Point::new(50.0, 50.0),
            Point::new(70.0, 20.0),
        ];
        let path = catmull_rom(&punti, 1.0);
        for p in &punti {
            let vicino = path
                .iter()
                .any(|q| (q.x - p.x).abs() < 1.5 && (q.y - p.y).abs() < 1.5);
            assert!(vicino, "la spline non passa vicino a ({}, {})", p.x, p.y);
        }
    }

    #[test]
    fn la_stabilizzazione_riduce_il_tremolio() {
        // Ingresso a zig-zag: segno di mano tremante.
        let punti: Vec<Point> = (0..24)
            .map(|i| {
                let x = 10.0 + i as f32 * 2.0;
                let y = 50.0 + if i % 2 == 0 { -4.0 } else { 4.0 };
                Point::new(x, y)
            })
            .collect();

        let variazione = |pts: &[Point]| {
            pts.windows(2)
                .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
                .sum::<f32>()
        };
        let grezzo = variazione(&punti);
        let liscio = variazione(&stabilise(&punti, 90.0));
        assert!(
            liscio < grezzo * 0.75,
            "stabilizzato {liscio} non è più liscio di {grezzo}"
        );
        // il punto di partenza non si muove
        assert_eq!(stabilise(&punti, 90.0)[0], punti[0]);
    }

    #[test]
    fn la_spaziatura_controlla_il_numero_di_timbri() {
        let linea = |spacing: f32| {
            let mut s = Stroke::new(Brush::new(10.0).spacing(spacing));
            s.add(Point::new(10.0, 32.0));
            s.add(Point::new(54.0, 32.0));
            s.stamps().len()
        };
        let fitto = linea(0.1);
        let medio = linea(0.5);
        let rado = linea(1.0);
        assert!(
            fitto > medio,
            "0.1 ({fitto}) deve avere più timbri di 0.5 ({medio})"
        );
        assert!(
            medio > rado,
            "0.5 ({medio}) deve avere più timbri di 1.0 ({rado})"
        );
        // 44 pixel di percorso, passo 1 -> circa 44 timbri
        assert!(
            (40..=46).contains(&fitto),
            "timbri con spacing 0.1: {fitto}"
        );
        // passo 10 -> circa 5 timbri
        assert!((4..=6).contains(&rado), "timbri con spacing 1.0: {rado}");
    }

    #[test]
    fn a_spaziatura_alta_i_timbri_sono_separati() {
        // Il buco più lungo su una riga: a 1.0 sono cuciture di un pixel
        // (cerchi tangenti), a 2.0 sono spazi veri larghi un diametro.
        let buco_piu_lungo = |spacing: f32| {
            let mut s = Stroke::new(Brush::new(8.0).spacing(spacing).hardness(1.0));
            s.add(Point::new(8.0, 16.0));
            s.add(Point::new(56.0, 16.0));
            let out = s.render(64, 32);
            let mut peggiore = 0;
            let mut corrente = 0;
            for x in 4..60 {
                if alpha_at(&out, 64, x, 16) == 0 {
                    corrente += 1;
                    peggiore = peggiore.max(corrente);
                } else {
                    corrente = 0;
                }
            }
            peggiore
        };

        let affiancati = buco_piu_lungo(1.0);
        assert!(
            affiancati <= 2,
            "a 1.0 i timbri si toccano: buco di {affiancati} pixel"
        );

        let puntinato = buco_piu_lungo(2.0);
        assert!(
            puntinato >= 6,
            "a 2.0 il buco deve essere quasi un diametro: {puntinato} pixel"
        );
    }

    #[test]
    fn la_pressione_cambia_linearmente_lo_spessore() {
        let area = |pressure: f32| {
            let mut s = Stroke::new(Brush::new(24.0).hardness(1.0));
            s.add(Point::new(32.0, 32.0).with_pressure(pressure));
            coperti(&s.render(64, 64))
        };
        let sottile = area(0.0);
        let medio = area(0.5);
        let pieno = area(1.0);
        assert!(
            sottile < medio && medio < pieno,
            "{sottile} < {medio} < {pieno}"
        );
        // a pressione 0 il raggio è il 15% del massimo -> area ~2.25%
        let rapporto = sottile as f32 / pieno as f32;
        assert!(
            rapporto < 0.05,
            "a pressione 0 il tratto deve essere sottile: {rapporto}"
        );
        // il raggio cresce in modo lineare, quindi l'area come il quadrato
        let atteso = (0.15 + 0.85 * 0.5f32).powi(2);
        let misurato = medio as f32 / pieno as f32;
        assert!(
            (misurato - atteso).abs() < 0.06,
            "atteso {atteso}, misurato {misurato}"
        );
    }

    #[test]
    fn hardness_0_sfuma_il_bordo_e_hardness_1_lo_rende_netto() {
        let intermedi = |hardness: f32| {
            let mut s = Stroke::new(Brush::new(24.0).hardness(hardness));
            s.add(Point::new(32.0, 32.0));
            s.render(64, 64)
                .chunks_exact(4)
                .filter(|p| p[3] > 0 && p[3] < 255)
                .count()
        };
        let sfumato = intermedi(0.0);
        let netto = intermedi(1.0);
        assert!(
            sfumato > 50,
            "hardness 0 deve avere un gradiente: {sfumato}"
        );
        assert_eq!(
            netto, 0,
            "hardness 1 non deve avere pixel intermedi: {netto}"
        );
    }

    #[test]
    fn il_blend_mode_cambia_il_risultato_su_un_fondo_noto() {
        let tratto = |blend: BlendMode| {
            let mut s = Stroke::new(Brush::new(8.0).hardness(1.0).color([0, 0, 0]).blend(blend));
            s.add(Point::new(4.0, 4.0));
            let mut dst = canvas(8, 8);
            s.render_over(&mut dst, 8, 8);
            px(&dst, 8, 4, 4)
        };

        assert_eq!(tratto(BlendMode::Normal), [0, 0, 0, 255], "nero su bianco");
        assert_eq!(
            tratto(BlendMode::Multiply),
            [0, 0, 0, 255],
            "moltiplica per nero"
        );
        assert_eq!(
            tratto(BlendMode::Screen),
            [255, 255, 255, 255],
            "schiarisci nero su bianco"
        );
    }

    #[test]
    fn l_opacita_del_pennello_media_il_tratto() {
        let mut s = Stroke::new(Brush::new(8.0).hardness(1.0).opacity(0.5).color([0, 0, 0]));
        s.add(Point::new(4.0, 4.0));
        let mut dst = canvas(8, 8);
        s.render_over(&mut dst, 8, 8);
        // 128/255 di nero su bianco: 127 o 128 a seconda dell'arrotondamento
        let p = px(&dst, 8, 4, 4);
        assert!(
            (p[0] as i32 - 128).abs() <= 1,
            "il grigio medio deve stare intorno a 128: {p:?}"
        );
        assert_eq!(p[3], 255);

        let mut trasparente = Stroke::new(Brush::new(8.0).opacity(0.0));
        trasparente.add(Point::new(4.0, 4.0));
        assert!(trasparente.render(8, 8).iter().all(|b| *b == 0));
    }

    #[test]
    fn il_tilt_schiaccia_il_timbro_in_ellisse() {
        // Con tilt sull'asse X la punta si schiaccia in verticale: è l'altezza
        // a cambiare, non la larghezza.
        let altezza = |tilt: f32| {
            let mut s = Stroke::new(Brush::new(30.0).hardness(1.0));
            s.add(Point::new(32.0, 32.0).with_tilt(tilt, 0.0));
            let out = s.render(64, 64);
            (0..64).filter(|y| alpha_at(&out, 64, 32, *y) > 0).count()
        };
        let tondo = altezza(0.0);
        let inclinato = altezza(90.0);
        assert!(
            inclinato < tondo,
            "il tilt deve stringere: {inclinato} < {tondo}"
        );
        assert!(inclinato > tondo / 2, "ma non annullarlo: {inclinato}");

        // la larghezza resta quella del pennello
        let mut s = Stroke::new(Brush::new(30.0).hardness(1.0));
        s.add(Point::new(32.0, 32.0).with_tilt(90.0, 0.0));
        let out = s.render(64, 64);
        let larghezza = (0..64).filter(|x| alpha_at(&out, 64, *x, 32) > 0).count();
        assert!(
            larghezza >= tondo - 2,
            "la larghezza non deve cambiare: {larghezza}"
        );
    }

    #[test]
    fn il_tilt_a_45_gradi_da_un_valore_intermedio() {
        // Il rapporto è lineare: 45° su 90° -> normalised 0.5 -> ratio 0.8.
        // Il test rende esplicito il contratto, così un cambio di formula
        // (per esempio coseno) non passa silenziosamente.
        let altezza = |tilt: f32| {
            let mut s = Stroke::new(Brush::new(30.0).hardness(1.0));
            s.add(Point::new(32.0, 32.0).with_tilt(tilt, 0.0));
            let out = s.render(64, 64);
            (0..64).filter(|y| alpha_at(&out, 64, 32, *y) > 0).count()
        };
        let pieno = altezza(0.0) as f32;
        let mezzo = altezza(45.0) as f32;
        let rapporto = mezzo / pieno;
        assert!(
            (rapporto - 0.8).abs() < 0.1,
            "a 45° il rapporto atteso è 0.8, misurato {rapporto}"
        );
    }

    #[test]
    fn un_tratto_fuori_dalla_tela_non_va_in_panico() {
        let mut s = Stroke::new(Brush::new(10.0));
        s.add(Point::new(-50.0, -50.0));
        s.add(Point::new(-40.0, -40.0));
        let out = s.render(16, 16);
        assert_eq!(out.len(), 16 * 16 * 4);
        assert!(out.iter().all(|b| *b == 0));
    }

    #[test]
    fn il_rendering_e_deterministico() {
        let mut s = Stroke::new(Brush::new(12.0));
        for i in 0..10 {
            s.add(Point::new(
                10.0 + i as f32 * 4.0,
                32.0 + (i as f32).sin() * 6.0,
            ));
        }
        assert_eq!(s.render(64, 64), s.render(64, 64));
    }

    #[test]
    fn mille_punti_in_meno_di_50_ms() {
        let mut s = Stroke::new(Brush::new(18.0).spacing(0.25).stabilize(40.0));
        for i in 0..1000 {
            let t = i as f32 / 999.0;
            s.add(Point::new(
                40.0 + t * 430.0,
                256.0 + (t * 8.0 * std::f32::consts::PI).sin() * 90.0,
            ));
        }
        let inizio = Instant::now();
        let out = s.render(512, 512);
        let ms = inizio.elapsed().as_secs_f64() * 1000.0;
        assert!(!out.is_empty());
        println!("1000 punti su 512x512: {ms:.2} ms");

        if cfg!(debug_assertions) {
            assert!(ms < 400.0, "in debug il limite è morbido: {ms:.2} ms");
        } else {
            assert!(ms < 50.0, "obiettivo di progetto: {ms:.2} ms");
        }
    }

    // ---------------------------------------------------------------------
    // Dirty rect: il tratto occupa solo la regione che tocca
    // ---------------------------------------------------------------------

    fn tratto_a_onda() -> Stroke {
        let mut s = Stroke::new(Brush::new(14.0).hardness(0.6).color([10, 20, 30]));
        for i in 0..20 {
            let t = i as f32 / 19.0;
            s.add(Point::new(
                50.0 + t * 100.0,
                60.0 + (t * 3.0 * std::f32::consts::PI).sin() * 25.0,
            ));
        }
        s
    }

    #[test]
    fn render_bbox_e_equivalente_a_render_ritagliato() {
        let stroke = tratto_a_onda();
        let (cw, ch) = (256u32, 128u32);

        let pieno = stroke.render(cw, ch);
        let tile = stroke.render_bbox(cw, ch);
        assert!(!tile.is_empty(), "il tratto è dentro la tela");

        // ritaglia il buffer pieno alla regione del tile e confronta byte per byte
        for row in 0..tile.height {
            for col in 0..tile.width {
                let x = tile.offset_x as u32 + col;
                let y = tile.offset_y as u32 + row;
                let i = ((y * cw + x) * 4) as usize;
                let j = ((row * tile.width + col) * 4) as usize;
                assert_eq!(
                    &pieno[i..i + 4],
                    &tile.rgba[j..j + 4],
                    "differenza al pixel ({x}, {y})"
                );
            }
        }

        // e fuori dal tile il buffer pieno è vuoto
        for y in 0..ch {
            for x in 0..cw {
                let dentro = x as i32 >= tile.offset_x
                    && (x as i32) < tile.offset_x + tile.width as i32
                    && y as i32 >= tile.offset_y
                    && (y as i32) < tile.offset_y + tile.height as i32;
                if !dentro {
                    let i = ((y * cw + x) * 4 + 3) as usize;
                    assert_eq!(pieno[i], 0, "alpha fuori dal tile in ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn il_bbox_di_uno_stroke_fuori_tela_e_vuoto() {
        let mut s = Stroke::new(Brush::new(10.0));
        s.add(Point::new(-100.0, -100.0));
        s.add(Point::new(-80.0, -80.0));
        let tile = s.render_bbox(64, 64);
        assert!(tile.is_empty(), "nessun pixel dentro la tela");
        assert_eq!(tile.width, 0);
        assert_eq!(tile.canvas_w, 64);
        assert!(tile.rgba.is_empty());
    }

    #[test]
    fn il_bbox_di_uno_stroke_parzialmente_fuori_tela_e_intersecato() {
        // punto a x=-10 con raggio 30: la metà sinistra è fuori dalla tela
        let mut s = Stroke::new(Brush::new(60.0).hardness(1.0));
        s.add(Point::new(-10.0, 50.0));
        let tile = s.render_bbox(100, 100);
        assert_eq!(tile.offset_x, 0, "il ritaglio parte dal bordo della tela");
        // il timbro arriva fino a x = -10 + 30 + 1 = 21
        assert!(
            (19..=23).contains(&(tile.width as i32)),
            "larghezza del ritaglio: {}",
            tile.width
        );
        assert!(
            tile.offset_y <= 25,
            "offset verticale coerente: {}",
            tile.offset_y
        );
    }

    #[test]
    fn il_bbox_di_un_singolo_punto_e_un_quadrato_del_diametro() {
        let mut s = Stroke::new(Brush::new(40.0).hardness(1.0));
        s.add(Point::new(100.0, 100.0));
        let tile = s.render_bbox(200, 200);
        // raggio 20 + 1 di margine per lato -> circa 42
        assert!((40..=44).contains(&tile.width), "larghezza {}", tile.width);
        assert!((40..=44).contains(&tile.height), "altezza {}", tile.height);
        assert_eq!(tile.byte_len(), (tile.width * tile.height * 4) as usize);
    }

    #[test]
    fn il_bbox_copre_tutti_i_timbri_del_tratto() {
        let stroke = tratto_a_onda();
        let (cw, ch) = (256u32, 128u32);
        let tile = stroke.render_bbox(cw, ch);
        let x0 = tile.offset_x as f32;
        let y0 = tile.offset_y as f32;
        let x1 = x0 + tile.width as f32;
        let y1 = y0 + tile.height as f32;
        for stamp in stroke.stamps() {
            let e = stamp.radius;
            assert!(
                stamp.x - e >= x0 - 1.0 && stamp.x + e <= x1 + 1.0,
                "timbro a x={} fuori dal ritaglio [{x0}, {x1}]",
                stamp.x
            );
            assert!(
                stamp.y - e >= y0 - 1.0 && stamp.y + e <= y1 + 1.0,
                "timbro a y={} fuori dal ritaglio [{y0}, {y1}]",
                stamp.y
            );
        }
    }

    #[test]
    fn uno_stroke_piccolo_su_canvas_grande_e_veloce() {
        // Il caso realistico: un dettaglio piccolo su una foto enorme.
        let mut s = Stroke::new(Brush::new(12.0).spacing(0.2));
        for i in 0..50 {
            let t = i as f32 / 49.0;
            s.add(Point::new(
                1950.0 + t * 100.0,
                1950.0 + (t * 2.0 * std::f32::consts::PI).sin() * 50.0,
            ));
        }

        let inizio = Instant::now();
        let tile = s.render_bbox(4000, 4000);
        let ms = inizio.elapsed().as_secs_f64() * 1000.0;
        println!(
            "tratto 100x100 su tela 4000x4000: {ms:.3} ms, {} KB",
            tile.byte_len() / 1024
        );

        assert!(!tile.is_empty());
        // la prova che il ritaglio funziona: il buffer è piccolo, non 64 MB
        assert!(
            tile.byte_len() <= 140 * 140 * 4,
            "il buffer deve essere ritagliato: {} byte",
            tile.byte_len()
        );

        if cfg!(debug_assertions) {
            assert!(ms < 50.0, "in debug il limite è morbido: {ms:.2} ms");
        } else {
            assert!(ms < 5.0, "obiettivo di progetto: {ms:.2} ms");
        }
    }
}
