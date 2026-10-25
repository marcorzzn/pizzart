//! Algebra lineare minima per il compositing 2D.
//!
//! Una sola struttura, `Mat4`, in **column-major**: è il layout che wgpu si
//! aspetta quando la matrice viene passata come uniform a uno shader WGSL
//! (`mat4x4<f32>`). Tenere qui questa convenzione evita il classico bug di
//! trasposizione quando il codice passa dalla CPU alla GPU.

/// Matrice 4x4 in column-major: l'elemento della colonna `c`, riga `r` è `m[c * 4 + r]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Default for Mat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        m: [
            1.0, 0.0, 0.0, 0.0, // colonna 0
            0.0, 1.0, 0.0, 0.0, // colonna 1
            0.0, 0.0, 1.0, 0.0, // colonna 2
            0.0, 0.0, 0.0, 1.0, // colonna 3
        ],
    };

    /// Affine 2D: `x' = a*x + c*y + tx`, `y' = b*x + d*y + ty`.
    ///
    /// `a,b` formano la prima colonna (immagine dell'asse X), `c,d` la seconda.
    pub fn affine(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        let mut m = Self::IDENTITY.m;
        m[0] = a;
        m[1] = b;
        m[4] = c;
        m[5] = d;
        m[12] = tx;
        m[13] = ty;
        Self { m }
    }

    pub fn translation(tx: f32, ty: f32) -> Self {
        Self::affine(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self::affine(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    /// Rotazione in radianti; positiva = oraria con y verso il basso.
    pub fn rotation(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self::affine(c, s, -s, c, 0.0, 0.0)
    }

    /// `self * rhs`: applica prima `rhs`, poi `self`.
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let mut out = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut acc = 0.0;
                for k in 0..4 {
                    acc += self.m[k * 4 + row] * rhs.m[col * 4 + k];
                }
                out[col * 4 + row] = acc;
            }
        }
        Mat4 { m: out }
    }

    /// Trasforma un punto 2D (assume `w = 1`).
    pub fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.m[0] * x + self.m[4] * y + self.m[12],
            self.m[1] * x + self.m[5] * y + self.m[13],
        )
    }

    /// Inversa di una affine 2D. `None` se la matrice è degenere (determinante ~0).
    ///
    /// Serve per l'hit-testing: dato un punto del documento, trovare il pixel del layer.
    pub fn inverse_affine(&self) -> Option<Mat4> {
        // Estrarre i coefficienti dalla prima/seconda colonna e dalla traslazione.
        let a = self.m[0];
        let b = self.m[1];
        let c = self.m[4];
        let d = self.m[5];
        let tx = self.m[12];
        let ty = self.m[13];

        let det = a * d - b * c;
        if det.abs() <= f32::EPSILON {
            return None;
        }
        let inv = 1.0 / det;
        // inversa di [[a, c],[b, d]] = 1/det * [[d, -c],[-b, a]]
        let ia = d * inv;
        let ib = -b * inv;
        let ic = -c * inv;
        let id = a * inv;
        // traslazione invertita: -R^-1 * t
        let itx = -(ia * tx + ic * ty);
        let ity = -(ib * tx + id * ty);
        Some(Self::affine(ia, ib, ic, id, itx, ity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn identity_lascia_i_punti() {
        let p = Mat4::IDENTITY.map_point(3.0, -7.0);
        approx(p.0, 3.0);
        approx(p.1, -7.0);
    }

    #[test]
    fn traslazione_e_scala() {
        let p = Mat4::translation(10.0, 20.0).map_point(1.0, 1.0);
        approx(p.0, 11.0);
        approx(p.1, 21.0);
        let q = Mat4::scale(2.0, 3.0).map_point(4.0, 5.0);
        approx(q.0, 8.0);
        approx(q.1, 15.0);
    }

    #[test]
    fn rotazione_di_90_gradi() {
        let p = Mat4::rotation(std::f32::consts::FRAC_PI_2).map_point(1.0, 0.0);
        approx(p.0, 0.0);
        approx(p.1, 1.0);
    }

    #[test]
    fn ordine_di_moltiplicazione_scala_poi_trasla() {
        // scale * translation: prima trasla, poi scala -> (1+10)*2
        let m = Mat4::scale(2.0, 2.0).mul(&Mat4::translation(10.0, 0.0));
        approx(m.map_point(1.0, 0.0).0, 22.0);
        // translation * scale: prima scala, poi trasla -> 1*2+10
        let n = Mat4::translation(10.0, 0.0).mul(&Mat4::scale(2.0, 2.0));
        approx(n.map_point(1.0, 0.0).0, 12.0);
    }

    #[test]
    fn inversa_annulla_la_trasformazione() {
        let m = Mat4::translation(30.0, -12.0)
            .mul(&Mat4::rotation(0.6))
            .mul(&Mat4::scale(2.5, 0.5));
        let inv = m.inverse_affine().expect("invertibile");
        let (x, y) = m.map_point(7.0, 9.0);
        let (bx, by) = inv.map_point(x, y);
        approx(bx, 7.0);
        approx(by, 9.0);
    }

    #[test]
    fn matrice_degenere_non_ha_inversa() {
        assert!(Mat4::scale(0.0, 1.0).inverse_affine().is_none());
    }
}
