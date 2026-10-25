//! Il manifest del progetto: la parte leggibile del file.
//!
//! Tutto ciò che descrive il documento senza essere pixel sta qui. I pixel
//! stanno in `assets/<uuid>.bin`, così il manifest resta piccolo e ispezionabile
//! anche con un documento da centinaia di megabyte.

use serde::{Deserialize, Serialize};

/// Versione dello schema scritta in questo build.
pub const SCHEMA_VERSION: u32 = 1;

/// Radice del manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    /// Con cosa è stato scritto: aiuta a capire i file vecchi.
    pub created_with: String,
    pub document: DocumentMeta,
    /// `"reference"` o `"embed"`. Oggi è sempre `reference`: il testo è
    /// rasterizzato in un layer, quindi nessun nodo riferisce un font.
    pub font_strategy: String,
    /// Tutti i nodi, piatti: la gerarchia è nei `children`.
    pub nodes: Vec<NodeEntry>,
    pub assets: Vec<AssetEntry>,
    /// Font referenziati o inclusi (vuoto finché non esistono layer di testo).
    #[serde(default)]
    pub fonts: Vec<FontEntry>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub width: u32,
    pub height: u32,
}

/// Un nodo: layer o gruppo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeEntry {
    /// Identificativo stabile: non cambia tra save e load.
    pub uuid: String,
    /// `"layer"` o `"group"`.
    pub kind: String,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    /// Codice numerico del blend, lo stesso usato dal renderer.
    pub blend: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_uuid: Option<String>,
    /// Solo per i layer: i gruppi non hanno trasformazione propria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<TransformEntry>,
    /// Solo per i layer: riferimento al blob dei pixel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixels_uuid: Option<String>,
    /// Solo per i gruppi: figli dal fondo alla cima.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TransformEntry {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
}

/// Un blob binario dentro lo zip.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetEntry {
    pub uuid: String,
    /// `"pixels"` o `"mask"`.
    pub kind: String,
    /// `"raw"` o `"zstd"`.
    pub codec: String,
    /// Dimensione **decompressa** in byte.
    pub byte_size: u64,
    pub width: u32,
    pub height: u32,
    /// Percorso dentro lo zip.
    pub file: String,
}

/// Un font usato dal progetto.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontEntry {
    pub family: String,
    /// Hash del file: serve a riconoscere lo stesso font anche se rinominato.
    pub sha256: String,
    /// Percorso dell'asset, se il font è incluso nel progetto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_file: Option<String>,
    /// Percorso originale, se il font è solo referenziato.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn il_manifest_si_serializza_e_rilegge() {
        let m = Manifest {
            schema_version: SCHEMA_VERSION,
            created_with: "pizzart 0.1.0".into(),
            document: DocumentMeta {
                width: 16,
                height: 8,
            },
            font_strategy: "reference".into(),
            nodes: vec![NodeEntry {
                uuid: "550e8400-e29b-41d4-a716-446655440000".into(),
                kind: "layer".into(),
                name: "fondo".into(),
                visible: true,
                opacity: 1.0,
                blend: 0,
                mask_uuid: None,
                transform: Some(TransformEntry {
                    x: 1.0,
                    y: 2.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    rotation: 0.0,
                }),
                pixels_uuid: Some("550e8400-e29b-41d4-a716-446655440001".into()),
                children: Vec::new(),
            }],
            assets: vec![AssetEntry {
                uuid: "550e8400-e29b-41d4-a716-446655440001".into(),
                kind: "pixels".into(),
                codec: "zstd".into(),
                byte_size: 512,
                width: 16,
                height: 8,
                file: "assets/x.bin".into(),
            }],
            fonts: Vec::new(),
        };
        let json = serde_json::to_string(&m).expect("serializza");
        let riletto: Manifest = serde_json::from_str(&json).expect("deserializza");
        assert_eq!(riletto.document.width, 16);
        assert_eq!(riletto.nodes.len(), 1);
        assert!(riletto.nodes[0].children.is_empty(), "children vuoto è omesso");
        assert!(
            !json.contains("children"),
            "i campi vuoti non devono appesantire il manifest"
        );
    }
}
