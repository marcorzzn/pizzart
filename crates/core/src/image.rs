//! Un buffer RGBA8 **ritagliato** e posizionato dentro una tela più grande.
//!
//! È il mattone che permette di lavorare solo dove serve: uno stroke lungo
//! dieci pixel su una tela 4000×4000 non deve allocare 64 MB. Lo stesso tipo
//! servirà agli effetti e alle selezioni, che hanno lo stesso problema.

/// Pixel RGBA8 di una regione rettangolare, più la sua posizione nella tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedImage {
    /// `width * height * 4` byte, ordine RGBA.
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Angolo superiore sinistro della regione nella tela.
    pub offset_x: i32,
    pub offset_y: i32,
    /// Dimensioni della tela di riferimento (anche quando la regione è vuota).
    pub canvas_w: u32,
    pub canvas_h: u32,
}

impl PlacedImage {
    /// Regione vuota: nessun pixel, ma la tela di riferimento resta nota.
    pub fn empty(canvas_w: u32, canvas_h: u32) -> Self {
        Self {
            rgba: Vec::new(),
            width: 0,
            height: 0,
            offset_x: 0,
            offset_y: 0,
            canvas_w,
            canvas_h,
        }
    }

    /// C'è qualcosa da disegnare?
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.rgba.is_empty()
    }

    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }

    /// Copia la regione con un nuovo angolo di partenza nella tela.
    pub fn shifted(&self, dx: i32, dy: i32) -> Self {
        Self {
            rgba: self.rgba.clone(),
            width: self.width,
            height: self.height,
            offset_x: self.offset_x + dx,
            offset_y: self.offset_y + dy,
            canvas_w: self.canvas_w,
            canvas_h: self.canvas_h,
        }
    }

    /// Pixel della regione, in coordinate locali.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y as usize * self.width as usize) + x as usize) * 4;
        Some([
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ])
    }

    /// Copia la regione dentro una tela trasparente delle dimensioni dichiarate.
    ///
    /// Il risultato è identico a un buffer pieno in cui solo questa regione ha
    /// contenuto. Le parti che escono dalla tela vengono tagliate, non
    /// provocate in errore.
    pub fn blit_into(&self, dst: &mut [u8], dst_w: u32, dst_h: u32) {
        if self.is_empty() {
            return;
        }
        for row in 0..self.height {
            let dy = self.offset_y + row as i32;
            if dy < 0 || dy as u32 >= dst_h {
                continue;
            }
            for col in 0..self.width {
                let dx = self.offset_x + col as i32;
                if dx < 0 || dx as u32 >= dst_w {
                    continue;
                }
                let src = ((row as usize * self.width as usize) + col as usize) * 4;
                let out = ((dy as usize * dst_w as usize) + dx as usize) * 4;
                dst[out..out + 4].copy_from_slice(&self.rgba[src..src + 4]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tela(w: u32, h: u32) -> Vec<u8> {
        vec![0u8; (w * h * 4) as usize]
    }

    #[test]
    fn una_regione_vuota_non_ha_pixel_ma_conosce_la_tela() {
        let v = PlacedImage::empty(4000, 4000);
        assert!(v.is_empty());
        assert_eq!(v.byte_len(), 0);
        assert_eq!(v.canvas_w, 4000);
        assert_eq!(v.pixel(0, 0), None);
    }

    #[test]
    fn blit_posiziona_i_pixel_all_offset() {
        let img = PlacedImage {
            rgba: vec![9, 8, 7, 255, 6, 5, 4, 255],
            width: 2,
            height: 1,
            offset_x: 1,
            offset_y: 2,
            canvas_w: 4,
            canvas_h: 4,
        };
        let mut dst = tela(4, 4);
        img.blit_into(&mut dst, 4, 4);
        let at = |x: u32, y: u32| {
            let i = ((y * 4 + x) * 4) as usize;
            [dst[i], dst[i + 1], dst[i + 2], dst[i + 3]]
        };
        assert_eq!(at(1, 2), [9, 8, 7, 255]);
        assert_eq!(at(2, 2), [6, 5, 4, 255]);
        assert_eq!(at(0, 2), [0, 0, 0, 0], "a sinistra resta trasparente");
        assert_eq!(at(1, 1), [0, 0, 0, 0], "sopra resta trasparente");
    }

    #[test]
    fn blit_taglia_cio_che_esce_dalla_tela() {
        let img = PlacedImage {
            rgba: vec![255; 4 * 4 * 4],
            width: 4,
            height: 4,
            offset_x: -2,
            offset_y: -2,
            canvas_w: 2,
            canvas_h: 2,
        };
        let mut dst = tela(2, 2);
        img.blit_into(&mut dst, 2, 2);
        // solo il quadrante in basso a destra finisce dentro
        assert_eq!(&dst[0..4], &[255, 255, 255, 255]);
        assert_eq!(&dst[(2 + 1) * 4..(2 + 1) * 4 + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn blit_in_una_tela_troppo_piccola_non_va_in_panico() {
        let img = PlacedImage {
            rgba: vec![1; 4 * 4 * 4],
            width: 4,
            height: 4,
            offset_x: 10,
            offset_y: 10,
            canvas_w: 16,
            canvas_h: 16,
        };
        let mut dst = tela(4, 4);
        img.blit_into(&mut dst, 4, 4);
        assert!(dst.iter().all(|b| *b == 0));
    }

    #[test]
    fn shifted_sposta_senza_toccare_i_pixel() {
        let img = PlacedImage {
            rgba: vec![7; 4],
            width: 1,
            height: 1,
            offset_x: 2,
            offset_y: 3,
            canvas_w: 16,
            canvas_h: 16,
        };
        let spostata = img.shifted(10, 20);
        assert_eq!(spostata.offset_x, 12);
        assert_eq!(spostata.offset_y, 23);
        assert_eq!(spostata.rgba, img.rgba);
        assert_eq!(img.offset_x, 2, "l'originale non cambia");
    }
}
