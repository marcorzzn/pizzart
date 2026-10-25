//! Registro dei font: `bytes → validazione → handle`.
//!
//! Il registro non sa da dove arrivano i byte (file, rete, memoria): chi carica
//! fa l'I/O, qui si valida e si tiene l'handle. Un font scaricato da internet è
//! **input non fidato**, quindi ogni cosa che può costare cara viene fermata
//! prima del parsing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cosmic_text::{fontdb, FontSystem};

/// Dimensione massima accettata per un singolo file di font (20 MB).
///
/// Un font patologico da centinaia di MB bloccherebbe il parsing per secondi:
/// il limite è una difesa contro il DoS, non una preferenza estetica.
pub const MAX_FONT_SIZE: usize = 20 * 1024 * 1024;

/// Numero massimo di font caricabili dall'utente.
pub const MAX_FONTS: usize = 256;

/// Handle di un font nel registro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontId(pub u32);

/// Da dove arriva un font. Serve a mostrarlo all'utente e a distinguere
/// i font di sistema da quelli caricati.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontSource {
    /// Font fornito dal sistema operativo.
    System,
    /// File scelto dall'utente.
    File(PathBuf),
    /// Scaricato da un URL (il download lo fa l'app, non questa libreria).
    Url(String),
    /// Byte in memoria (`include_bytes!`, drag-and-drop, test).
    Memory,
}

/// Metadati di un font caricato.
#[derive(Clone, Debug)]
pub struct FontInfo {
    pub id: FontId,
    pub family: String,
    pub post_script_name: String,
    pub weight: u16,
    pub italic: bool,
    pub source: FontSource,
}

/// Errori di caricamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontError {
    /// Il file supera [`MAX_FONT_SIZE`].
    TooLarge { size: usize, max: usize },
    /// I primi byte non sono una firma di font conosciuta.
    InvalidMagic,
    /// Firma valida ma dati non interpretabili.
    Parse(String),
    /// Un font con la stessa famiglia è già registrato.
    AlreadyExists { family: String },
    /// Il registro ha raggiunto [`MAX_FONTS`].
    TooManyFonts { max: usize },
    /// Il file contiene una font collection con troppi font.
    TooManyFaces { found: usize, max: usize },
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::TooLarge { size, max } => {
                write!(f, "font di {size} byte, massimo {max}")
            }
            FontError::InvalidMagic => write!(f, "i byte non sono un font TTF/OTF/TTC"),
            FontError::Parse(e) => write!(f, "font non interpretabile: {e}"),
            FontError::AlreadyExists { family } => {
                write!(f, "un font della famiglia '{family}' è già registrato")
            }
            FontError::TooManyFonts { max } => write!(f, "raggiunto il limite di {max} font"),
            FontError::TooManyFaces { found, max } => {
                write!(f, "collection con {found} font, massimo {max}")
            }
        }
    }
}

impl std::error::Error for FontError {}

/// Numero massimo di font accettati in una singola collection (TTC).
pub const MAX_FACES_PER_FILE: usize = 64;

/// Controlla dimensione e firma **prima** di passare i byte al parser.
pub fn validate(bytes: &[u8]) -> Result<(), FontError> {
    if bytes.len() > MAX_FONT_SIZE {
        return Err(FontError::TooLarge {
            size: bytes.len(),
            max: MAX_FONT_SIZE,
        });
    }
    if bytes.len() < 12 {
        return Err(FontError::InvalidMagic);
    }
    let magic = &bytes[0..4];
    // TTF/OTF con contorni TrueType, OTF con contorni CFF, font collection.
    let ok = matches!(magic, [0x00, 0x01, 0x00, 0x00] | [0x74, 0x72, 0x75, 0x65])
        || magic == b"OTTO"
        || magic == b"ttcf";
    if !ok {
        return Err(FontError::InvalidMagic);
    }
    Ok(())
}

/// Registro dei font disponibili.
pub struct FontRegistry {
    system: FontSystem,
    fonts: BTreeMap<FontId, (fontdb::ID, FontInfo)>,
    next_id: u32,
    /// Font di sistema già enumerati come [`FontSource::System`].
    system_listed: bool,
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FontRegistry {
    /// Crea il registro e carica i font di sistema (fallback per i glifi mancanti).
    pub fn new() -> Self {
        Self {
            system: FontSystem::new(),
            fonts: BTreeMap::new(),
            next_id: 1,
            system_listed: false,
        }
    }

    /// Registra un font dai byte.
    ///
    /// Valida dimensione e firma, poi carica. Rifiuta i duplicati per famiglia:
    /// due font con lo stesso nome renderebbero ambigua la scelta all'utente.
    pub fn add(&mut self, bytes: Vec<u8>, source: FontSource) -> Result<FontId, FontError> {
        validate(&bytes)?;
        if self.fonts.len() >= MAX_FONTS {
            return Err(FontError::TooManyFonts { max: MAX_FONTS });
        }

        let prima: Vec<fontdb::ID> = self.system.db().faces().map(|f| f.id).collect();
        self.system.db_mut().load_font_data(bytes);
        let nuovi: Vec<fontdb::ID> = self
            .system
            .db()
            .faces()
            .map(|f| f.id)
            .filter(|id| !prima.contains(id))
            .collect();

        // Una collection (TTC) può contenere decine di volti: se ne accetta
        // solo un numero ragionevole, poi il resto viene scartato.
        if nuovi.len() > MAX_FACES_PER_FILE {
            for id in &nuovi {
                self.system.db_mut().remove_face(*id);
            }
            return Err(FontError::TooManyFaces {
                found: nuovi.len(),
                max: MAX_FACES_PER_FILE,
            });
        }
        let Some(&face_id) = nuovi.first() else {
            return Err(FontError::Parse(
                "nessun volto riconosciuto nel file".into(),
            ));
        };

        let face = self
            .system
            .db()
            .face(face_id)
            .ok_or_else(|| FontError::Parse("il parser non riconosce il font".into()))?;

        let family = face
            .families
            .first()
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| "Senza nome".to_string());

        if self.fonts.values().any(|(_, info)| info.family == family) {
            self.system.db_mut().remove_face(face_id);
            return Err(FontError::AlreadyExists { family });
        }

        let id = FontId(self.next_id);
        self.next_id += 1;
        self.fonts.insert(
            id,
            (
                face_id,
                FontInfo {
                    id,
                    family,
                    post_script_name: face.post_script_name.clone(),
                    weight: face.weight.0,
                    italic: matches!(face.style, fontdb::Style::Italic | fontdb::Style::Oblique),
                    source,
                },
            ),
        );
        Ok(id)
    }

    /// Registra un font leggendolo da un file.
    pub fn add_file(&mut self, path: impl Into<PathBuf>) -> Result<FontId, FontError> {
        let path = path.into();
        let bytes = std::fs::read(&path)
            .map_err(|e| FontError::Parse(format!("lettura di {}: {e}", path.display())))?;
        self.add(bytes, FontSource::File(path))
    }

    /// Rimuove un font caricato e libera le cache.
    pub fn remove(&mut self, id: FontId) -> bool {
        match self.fonts.remove(&id) {
            Some((face_id, _)) => {
                self.system.db_mut().remove_face(face_id);
                true
            }
            None => false,
        }
    }

    pub fn get(&self, id: FontId) -> Option<&FontInfo> {
        self.fonts.get(&id).map(|(_, info)| info)
    }

    /// Font caricati dall'utente o dall'app, in ordine di handle.
    pub fn list(&self) -> Vec<&FontInfo> {
        self.fonts.values().map(|(_, info)| info).collect()
    }

    /// Cerca per famiglia (case-insensitive), utile per una UI di scelta.
    pub fn find_family(&self, family: &str) -> Option<FontId> {
        self.fonts
            .values()
            .find(|(_, info)| info.family.eq_ignore_ascii_case(family))
            .map(|(_, info)| info.id)
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Elenca le famiglie note al sistema operativo (una volta sola).
    pub fn system_families(&mut self) -> Vec<String> {
        let mut families: Vec<String> = self
            .system
            .db()
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
            .collect();
        families.sort();
        families.dedup();
        self.system_listed = true;
        families
    }

    /// Accesso al motore di testo, per il rendering.
    pub fn system_mut(&mut self) -> &mut FontSystem {
        &mut self.system
    }

    pub fn system(&self) -> &FontSystem {
        &self.system
    }
}

/// Famiglia di ripiego se il font richiesto non esiste.
pub fn fallback_family(registry: &mut FontRegistry) -> String {
    registry
        .system_families()
        .first()
        .cloned()
        .unwrap_or_else(|| "sans-serif".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un font vero dal sistema, se esiste: i test non inventano byte.
    pub fn system_font_bytes() -> Option<Vec<u8>> {
        let candidates = [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\tahoma.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ];
        candidates.iter().find_map(|p| std::fs::read(p).ok())
    }

    #[test]
    fn font_valido_si_carica() {
        let Some(bytes) = system_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut reg = FontRegistry::new();
        let id = reg.add(bytes, FontSource::Memory).expect("font valido");
        let info = reg.get(id).expect("info presente");
        assert!(!info.family.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn font_invalido_rifiutato() {
        let mut reg = FontRegistry::new();
        let err = reg
            .add(b"questo non e' un font".to_vec(), FontSource::Memory)
            .unwrap_err();
        assert_eq!(err, FontError::InvalidMagic);
        assert!(
            reg.is_empty(),
            "un font rifiutato non deve restare in registro"
        );
    }

    #[test]
    fn font_troppo_grande_rifiutato() {
        let mut reg = FontRegistry::new();
        let mut bytes = vec![0u8; MAX_FONT_SIZE + 1];
        bytes[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        let err = reg.add(bytes, FontSource::Memory).unwrap_err();
        assert!(matches!(err, FontError::TooLarge { .. }));
    }

    #[test]
    fn due_font_con_stessa_famiglia_rifiutati() {
        let Some(bytes) = system_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut reg = FontRegistry::new();
        let id = reg.add(bytes.clone(), FontSource::Memory).expect("primo");
        let family = reg.get(id).unwrap().family.clone();

        let err = reg.add(bytes, FontSource::Memory).unwrap_err();
        assert_eq!(err, FontError::AlreadyExists { family });
        assert_eq!(reg.len(), 1, "il duplicato non deve essere registrato");
    }

    #[test]
    fn rimozione_font_libera_il_registro() {
        let Some(bytes) = system_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut reg = FontRegistry::new();
        let id = reg.add(bytes, FontSource::Memory).expect("font");
        assert!(reg.get(id).is_some());
        assert!(reg.remove(id));
        assert!(reg.get(id).is_none());
        assert!(reg.list().is_empty());
        assert!(
            !reg.remove(id),
            "rimuovere due volte non è un errore, ma non fa nulla"
        );
    }

    #[test]
    fn la_ricerca_per_famiglia_non_distingue_maiuscole() {
        let Some(bytes) = system_font_bytes() else {
            eprintln!("nessun font di sistema: test saltato");
            return;
        };
        let mut reg = FontRegistry::new();
        let id = reg.add(bytes, FontSource::Memory).expect("font");
        let family = reg.get(id).unwrap().family.clone();
        assert_eq!(reg.find_family(&family.to_uppercase()), Some(id));
        assert_eq!(reg.find_family("famiglia inesistente"), None);
    }
}
