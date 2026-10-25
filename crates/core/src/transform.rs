//! Trasformazione di un layer: posizione, scala, rotazione.
//!
//! Catena di coordinate, nell'ordine in cui i pixel ci passano dentro:
//!
//! ```text
//! pixel del layer  --(scala, rotazione, traslazione)-->  pixel del documento  --(NDC)-->  clip space
//! ```
//!
//! Le rotazioni sono in radianti e positive in senso orario, perché l'asse Y
//! dei pixel scende verso il basso.

use crate::math::Mat4;

/// Posizione, scala e rotazione di un layer dentro il documento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// Radianti, positivo = orario.
    pub rotation: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}

impl Transform {
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            ..Self::default()
        }
    }

    /// Da pixel del layer a pixel del documento.
    pub fn layer_to_doc(&self) -> Mat4 {
        Mat4::translation(self.x, self.y)
            .mul(&Mat4::rotation(self.rotation))
            .mul(&Mat4::scale(self.scale_x, self.scale_y))
    }

    /// Da pixel del layer a coordinate normalizzate del dispositivo (`[-1, 1]`),
    /// pronte per lo shader: `y` viene ribaltato perché nei pixel cresce in basso.
    pub fn to_matrix(&self, doc_w: u32, doc_h: u32) -> Mat4 {
        let ndc = Mat4::affine(
            2.0 / doc_w.max(1) as f32,
            0.0,
            0.0,
            -2.0 / doc_h.max(1) as f32,
            -1.0,
            1.0,
        );
        ndc.mul(&self.layer_to_doc())
    }

    /// Porta un punto del layer nello spazio del documento.
    pub fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        self.layer_to_doc().map_point(x, y)
    }

    /// Porta un punto del documento nello spazio del layer (hit-testing).
    /// `None` se la trasformazione è degenere (scala 0).
    pub fn doc_to_layer(&self) -> Option<Mat4> {
        self.layer_to_doc().inverse_affine()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn senza_trasformazione_i_punti_non_si_muovono() {
        let t = Transform::default();
        let (x, y) = t.map_point(12.0, 34.0);
        approx(x, 12.0);
        approx(y, 34.0);
    }

    #[test]
    fn il_centro_del_documento_finisce_nell_origine_ndc() {
        let t = Transform::default();
        let m = t.to_matrix(200, 200);
        let (x, y) = m.map_point(100.0, 100.0);
        approx(x, 0.0);
        approx(y, 0.0);
    }

    #[test]
    fn l_angolo_in_alto_a_sinistra_e_meno_uno_meno_uno() {
        let m = Transform::default().to_matrix(200, 200);
        let (x, y) = m.map_point(0.0, 0.0);
        approx(x, -1.0);
        approx(y, 1.0); // y=0 in pixel è il bordo superiore -> +1 in NDC
    }

    #[test]
    fn traslazione_di_50_pixel_vale_mezzo_documento_in_ndc() {
        let m = Transform::at(50.0, 0.0).to_matrix(100, 100);
        let (x, _) = m.map_point(0.0, 0.0);
        // x=50 pixel su 100 -> 2*50/100 - 1 = 0
        approx(x, 0.0);
    }

    #[test]
    fn rotazione_di_90_gradi_porta_il_punto_sull_asse_y() {
        let t = Transform {
            rotation: std::f32::consts::FRAC_PI_2,
            ..Transform::default()
        };
        let (x, y) = t.map_point(10.0, 0.0);
        approx(x, 0.0);
        approx(y, 10.0);
    }

    #[test]
    fn scala_e_ordine_della_catena() {
        // scala 2x poi traslazione di 10: (3,0) -> (6,0) -> (16,0)
        let t = Transform {
            x: 10.0,
            scale_x: 2.0,
            ..Transform::default()
        };
        let (x, _) = t.map_point(3.0, 0.0);
        approx(x, 16.0);
    }

    #[test]
    fn doc_to_layer_inverte_la_trasformazione() {
        let t = Transform {
            x: 30.0,
            y: -12.0,
            scale_x: 2.5,
            scale_y: 0.5,
            rotation: 0.6,
        };
        let (dx, dy) = t.map_point(7.0, 9.0);
        let inv = t.doc_to_layer().expect("invertibile");
        let (lx, ly) = inv.map_point(dx, dy);
        approx(lx, 7.0);
        approx(ly, 9.0);
    }

    #[test]
    fn scala_zero_non_ha_inversa() {
        let t = Transform {
            scale_x: 0.0,
            ..Transform::default()
        };
        assert!(t.doc_to_layer().is_none());
    }
}
