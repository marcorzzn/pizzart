//! # pizzart-project
//!
//! Salvataggio e caricamento di un progetto: uno zip con `manifest.json` e i
//! blob binari in `assets/`.
//!
//! ## Perché zip + manifest + asset separati
//!
//! Il manifest pesa pochi KB anche con un documento da centinaia di megabyte e
//! si legge con `unzip -p progetto.pizzart manifest.json`. I pixel non
//! appartengono al JSON: sono binari, e vanno compressi con un codec che sa fare
//! il suo mestiere.
//!
//! ## Identificativi
//!
//! In memoria un nodo ha un [`pizzart_core::LayerId`] (contatore veloce). Nel
//! file ha uno [`StableId`] a 16 byte, stabile tra sessioni e documenti. Al
//! caricamento il contatore riparte, l'id stabile no: è ciò che permette
//! copia-incolla tra progetti e template referenziabili.
//!
//! ## Compressione
//!
//! `Codec::Zstd` (livello 1 di default) sui pixel RGBA8, che si comprimono bene;
//! `Codec::Raw` per contenuti già compressi. Lo zip è solo il contenitore.
//!
//! ## Cosa questo formato non contiene ancora
//!
//! I font. Non per dimenticanza: il testo oggi viene rasterizzato in una tessera,
//! quindi **un layer non riferisce alcun font**. Il manifest ha già i campi
//! (`font_strategy`, `fonts`) perché il giorno in cui esisterà un layer di testo
//! il formato non debba cambiare versione.

#![forbid(unsafe_code)]

pub mod manifest;

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::Path;

use pizzart_core::{
    BlendMode, Document, Group, Layer, LayerId, Mask, MaskSource, Node, StableId, Transform,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub use manifest::{
    AssetEntry, DocumentMeta, FontEntry, Manifest, NodeEntry, TransformEntry, SCHEMA_VERSION,
};

/// Come trattare i font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStrategy {
    /// Solo nome e hash: il progetto resta leggero, il font deve esistere.
    Reference,
    /// I byte del font finiscono nel progetto.
    Embed,
}

/// Compressione dei blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// Nessuna compressione (contenuti già compressi).
    Raw,
    /// Zstd, livello indicato da [`SaveOptions::compress_level`].
    Zstd,
}

/// Opzioni di salvataggio.
#[derive(Clone, Copy, Debug)]
pub struct SaveOptions {
    pub font_strategy: FontStrategy,
    pub codec: Codec,
    /// Livello Zstd: 1 è il default (velocità), 0 equivale a `Raw`.
    pub compress_level: i32,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            font_strategy: FontStrategy::Reference,
            codec: Codec::Zstd,
            compress_level: 1,
        }
    }
}

/// Errori di salvataggio e caricamento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectError {
    Io(String),
    Manifest(String),
    Zip(String),
    Codec(String),
    /// Il file è stato scritto da una versione più recente.
    VersionMismatch { found: u32, max_supported: u32 },
    /// Un asset citato dal manifest non è nello zip.
    MissingAsset { uuid: String },
    /// Un font referenziato non è disponibile.
    MissingFont { family: String, hash: String },
    /// Manifest leggibile ma struttura incoerente.
    InvalidStructure(String),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::Io(e) => write!(f, "errore di I/O: {e}"),
            ProjectError::Manifest(e) => write!(f, "manifest non valido: {e}"),
            ProjectError::Zip(e) => write!(f, "archivio non valido: {e}"),
            ProjectError::Codec(e) => write!(f, "compressione: {e}"),
            ProjectError::VersionMismatch {
                found,
                max_supported,
            } => write!(
                f,
                "progetto con schema {found}, questa versione ne supporta fino a {max_supported}"
            ),
            ProjectError::MissingAsset { uuid } => write!(f, "asset mancante: {uuid}"),
            ProjectError::MissingFont { family, hash } => {
                write!(f, "font mancante: {family} ({hash})")
            }
            ProjectError::InvalidStructure(e) => write!(f, "struttura incoerente: {e}"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self {
        ProjectError::Io(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Salvataggio
// ---------------------------------------------------------------------------

/// Salva il documento in un file `.pizzart` (zip).
pub fn save(doc: &Document, path: &Path, options: &SaveOptions) -> Result<(), ProjectError> {
    let mut raccolta = Raccolta::default();
    raccogli(&doc.root, &mut raccolta, options);

    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        created_with: format!("pizzart {}", env!("CARGO_PKG_VERSION")),
        document: DocumentMeta {
            width: doc.width,
            height: doc.height,
        },
        font_strategy: match options.font_strategy {
            FontStrategy::Reference => "reference".into(),
            FontStrategy::Embed => "embed".into(),
        },
        nodes: raccolta.nodes,
        assets: raccolta.assets,
        fonts: Vec::new(),
    };

    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| ProjectError::Manifest(e.to_string()))?;

    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let memorizzato = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("manifest.json", memorizzato)
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    zip.write_all(json.as_bytes())?;

    for (nome, dati) in &raccolta.blob {
        zip.start_file(nome.clone(), memorizzato)
            .map_err(|e| ProjectError::Zip(e.to_string()))?;
        zip.write_all(dati)?;
    }

    zip.finish()
        .map_err(|e| ProjectError::Zip(e.to_string()))?;
    Ok(())
}

#[derive(Default)]
struct Raccolta {
    nodes: Vec<NodeEntry>,
    assets: Vec<AssetEntry>,
    /// Blob da scrivere nello zip: (percorso, byte da scrivere).
    blob: Vec<(String, Vec<u8>)>,
    /// Maschere già scritte, indicizzate per condivisione (`Arc`).
    maschere: HashMap<usize, String>,
}

fn raccogli(nodes: &[Node], out: &mut Raccolta, options: &SaveOptions) {
    for node in nodes {
        let uuid = node.stable().to_uuid_string();
        match node {
            Node::Layer(layer) => {
                let pixels_uuid = scrivi_asset(
                    &layer.pixels,
                    layer.width,
                    layer.height,
                    "pixels",
                    out,
                    options,
                );
                let mask_uuid = layer.mask.as_ref().map(|mask| {
                    // una maschera condivisa fra layer diventa **un solo** asset
                    let chiave = std::sync::Arc::as_ptr(mask) as usize;
                    if let Some(uuid) = out.maschere.get(&chiave) {
                        return uuid.clone();
                    }
                    let uuid =
                        scrivi_asset(&mask.alpha, mask.width, mask.height, "mask", out, options);
                    out.maschere.insert(chiave, uuid.clone());
                    uuid
                });

                out.nodes.push(NodeEntry {
                    uuid,
                    kind: "layer".into(),
                    name: layer.name.clone(),
                    visible: layer.visible,
                    opacity: layer.opacity,
                    blend: blend_code(layer.blend),
                    mask_uuid,
                    transform: Some(TransformEntry {
                        x: layer.transform.x,
                        y: layer.transform.y,
                        scale_x: layer.transform.scale_x,
                        scale_y: layer.transform.scale_y,
                        rotation: layer.transform.rotation,
                    }),
                    pixels_uuid: Some(pixels_uuid),
                    children: Vec::new(),
                });
            }
            Node::Group(group) => {
                raccogli(&group.children, out, options);
                let children = group
                    .children
                    .iter()
                    .map(|c| c.stable().to_uuid_string())
                    .collect();
                out.nodes.push(NodeEntry {
                    uuid,
                    kind: "group".into(),
                    name: group.name.clone(),
                    visible: group.visible,
                    opacity: group.opacity,
                    blend: blend_code(group.blend),
                    mask_uuid: None,
                    transform: None,
                    pixels_uuid: None,
                    children,
                });
            }
        }
    }
}

fn scrivi_asset(
    dati: &[u8],
    width: u32,
    height: u32,
    kind: &str,
    out: &mut Raccolta,
    options: &SaveOptions,
) -> String {
    let uuid = StableId::new().to_uuid_string();
    let file = format!("assets/{uuid}.bin");
    let (codec, da_scrivere) = match options.codec {
        Codec::Raw => ("raw", dati.to_vec()),
        Codec::Zstd => {
            let livello = options.compress_level.clamp(0, 22);
            if livello == 0 {
                ("raw", dati.to_vec())
            } else {
                match zstd::bulk::compress(dati, livello) {
                    Ok(c) => ("zstd", c),
                    // se la compressione fallisce non si perde il dato: meglio
                    // un file più grande che un progetto non salvabile
                    Err(_) => ("raw", dati.to_vec()),
                }
            }
        }
    };
    out.assets.push(AssetEntry {
        uuid: uuid.clone(),
        kind: kind.into(),
        codec: codec.into(),
        byte_size: dati.len() as u64,
        width,
        height,
        file: file.clone(),
    });
    out.blob.push((file, da_scrivere));
    uuid
}

fn blend_code(mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::ColorDodge => 6,
        BlendMode::ColorBurn => 7,
        BlendMode::HardLight => 8,
        BlendMode::SoftLight => 9,
        BlendMode::Difference => 10,
        BlendMode::Exclusion => 11,
    }
}

fn blend_from_code(code: u8) -> BlendMode {
    match code {
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::ColorDodge,
        7 => BlendMode::ColorBurn,
        8 => BlendMode::HardLight,
        9 => BlendMode::SoftLight,
        10 => BlendMode::Difference,
        11 => BlendMode::Exclusion,
        _ => BlendMode::Normal,
    }
}

// ---------------------------------------------------------------------------
// Caricamento
// ---------------------------------------------------------------------------

/// Carica un progetto da file.
///
/// Non serve il registro dei font: il testo è rasterizzato nei pixel, quindi il
/// documento non riferisce font. Quando esisterà un layer di testo, questo
/// ingresso prenderà il registro.
pub fn load(path: &Path) -> Result<Document, ProjectError> {
    let manifest = peek_manifest(path)?;

    if manifest.schema_version > SCHEMA_VERSION {
        return Err(ProjectError::VersionMismatch {
            found: manifest.schema_version,
            max_supported: SCHEMA_VERSION,
        });
    }

    let file = std::fs::File::open(path)?;
    let mut archivio = ZipArchive::new(file).map_err(|e| ProjectError::Zip(e.to_string()))?;

    // 1. leggi tutti gli asset, indicizzati per uuid
    let mut asset: HashMap<String, (Vec<u8>, u32, u32)> = HashMap::new();
    for voce in &manifest.assets {
        let mut dati = Vec::new();
        archivio
            .by_name(&voce.file)
            .map_err(|_| ProjectError::MissingAsset {
                uuid: voce.uuid.clone(),
            })?
            .read_to_end(&mut dati)?;
        let grezzi = if voce.codec == "zstd" {
            zstd::bulk::decompress(&dati, voce.byte_size as usize)
                .map_err(|e| ProjectError::Codec(e.to_string()))?
        } else {
            dati
        };
        if grezzi.len() as u64 != voce.byte_size {
            return Err(ProjectError::InvalidStructure(format!(
                "asset {}: {} byte invece di {}",
                voce.uuid,
                grezzi.len(),
                voce.byte_size
            )));
        }
        asset.insert(voce.uuid.clone(), (grezzi, voce.width, voce.height));
    }

    // 2. indice dei nodi e insieme dei figli, per riconoscere le radici
    let mut per_uuid: HashMap<String, &NodeEntry> = HashMap::new();
    let mut figli: HashSet<String> = HashSet::new();
    for voce in &manifest.nodes {
        if per_uuid.insert(voce.uuid.clone(), voce).is_some() {
            return Err(ProjectError::InvalidStructure(format!(
                "uuid duplicato: {}",
                voce.uuid
            )));
        }
        for figlio in &voce.children {
            figli.insert(figlio.clone());
        }
    }

    // stesso `Arc<Mask>` per lo stesso uuid: la condivisione sopravvive al salvataggio
    let mut maschere: HashMap<String, std::sync::Arc<Mask>> = HashMap::new();
    let mut radice = Vec::new();
    for voce in &manifest.nodes {
        if !figli.contains(&voce.uuid) {
            radice.push(costruisci(voce, &per_uuid, &asset, &mut maschere)?);
        }
    }
    if radice.is_empty() && !manifest.nodes.is_empty() {
        return Err(ProjectError::InvalidStructure(
            "nessuna radice: ciclo nei riferimenti?".into(),
        ));
    }

    Ok(Document {
        width: manifest.document.width,
        height: manifest.document.height,
        root: radice,
        history: pizzart_core::history::History::new(pizzart_core::history::DEFAULT_HISTORY_LIMIT),
    })
}

fn costruisci(
    voce: &NodeEntry,
    per_uuid: &HashMap<String, &NodeEntry>,
    asset: &HashMap<String, (Vec<u8>, u32, u32)>,
    maschere: &mut HashMap<String, std::sync::Arc<Mask>>,
) -> Result<Node, ProjectError> {
    let stable = StableId::parse_uuid_string(&voce.uuid)
        .ok_or_else(|| ProjectError::InvalidStructure(format!("uuid non valido: {}", voce.uuid)))?;

    let mask = match &voce.mask_uuid {
        Some(uuid) => {
            if let Some(gia) = maschere.get(uuid) {
                Some(gia.clone())
            } else {
                let (dati, w, h) = asset.get(uuid).ok_or(ProjectError::MissingAsset {
                    uuid: uuid.clone(),
                })?;
                let mask = std::sync::Arc::new(Mask::new(
                    dati.clone(),
                    *w,
                    *h,
                    MaskSource::FromTile,
                ));
                maschere.insert(uuid.clone(), mask.clone());
                Some(mask)
            }
        }
        None => None,
    };

    match voce.kind.as_str() {
        "layer" => {
            let uuid = voce.pixels_uuid.as_ref().ok_or_else(|| {
                ProjectError::InvalidStructure(format!("layer {} senza pixel", voce.uuid))
            })?;
            let (dati, w, h) = asset.get(uuid).ok_or(ProjectError::MissingAsset {
                uuid: uuid.clone(),
            })?;
            let t = voce.transform.unwrap_or(TransformEntry {
                x: 0.0,
                y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
            });
            Ok(Node::Layer(Layer {
                id: LayerId::new(),
                stable,
                name: voce.name.clone(),
                visible: voce.visible,
                width: *w,
                height: *h,
                pixels: dati.clone(),
                transform: Transform {
                    x: t.x,
                    y: t.y,
                    scale_x: t.scale_x,
                    scale_y: t.scale_y,
                    rotation: t.rotation,
                },
                blend: blend_from_code(voce.blend),
                opacity: voce.opacity,
                mask,
            }))
        }
        "group" => {
            let mut children = Vec::new();
            for figlio in &voce.children {
                let nodo = per_uuid.get(figlio).ok_or_else(|| {
                    ProjectError::InvalidStructure(format!("figlio mancante: {figlio}"))
                })?;
                children.push(costruisci(nodo, per_uuid, asset, maschere)?);
            }
            Ok(Node::Group(Group {
                id: LayerId::new(),
                stable,
                name: voce.name.clone(),
                visible: voce.visible,
                opacity: voce.opacity,
                blend: blend_from_code(voce.blend),
                mask,
                children,
            }))
        }
        altro => Err(ProjectError::InvalidStructure(format!(
            "tipo di nodo sconosciuto: {altro}"
        ))),
    }
}

/// Legge solo il manifest: ispeziona un progetto senza caricarne i pixel.
pub fn peek_manifest(path: &Path) -> Result<Manifest, ProjectError> {
    let file = std::fs::File::open(path)?;
    let mut archivio = ZipArchive::new(file).map_err(|e| ProjectError::Zip(e.to_string()))?;
    let mut voce = archivio
        .by_name("manifest.json")
        .map_err(|_| ProjectError::Manifest("manifest.json assente".into()))?;
    let mut testo = String::new();
    voce.read_to_string(&mut testo)?;
    serde_json::from_str(&testo).map_err(|e| ProjectError::Manifest(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pizzart_core::EllipseSelection;

    /// `Document` non implementa Debug (la storia non lo consente): per i test
    /// sugli errori si prende il lato `Err` senza `unwrap_err`.
    fn errore(r: Result<Document, ProjectError>) -> ProjectError {
        r.err().expect("errore atteso")
    }
    fn temp(nome: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("pizzart-test-{nome}-{}.pizzart", StableId::new()));
        p
    }

    fn layer(name: &str, w: u32, h: u32, colore: [u8; 4]) -> Layer {
        Layer::filled(name, w, h, colore)
    }

    fn gradient(w: u32, h: u32, seed: u32) -> Layer {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let t = ((x * 255 / w.max(1)) as u8).wrapping_add(seed as u8);
                let u = (y * 255 / h.max(1)) as u8;
                rgba.extend_from_slice(&[t, u, (t / 2).wrapping_add(u / 2), 255]);
            }
        }
        Layer::new("gradiente", w, h, rgba)
    }

    #[test]
    fn save_load_roundtrip_documento_vuoto() {
        let path = temp("vuoto");
        let doc = Document::new(100, 50);
        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");
        assert_eq!((letto.width, letto.height), (100, 50));
        assert_eq!(letto.layer_count(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_load_roundtrip_con_un_layer() {
        let path = temp("layer");
        let mut doc = Document::new(16, 12);
        let originale = gradient(16, 12, 7);
        doc.add_layer(originale.clone());
        save(&doc, &path, &SaveOptions::default()).expect("save");

        let letto = load(&path).expect("load");
        assert_eq!(letto.layer_count(), 1);
        let l = letto.layers()[0];
        assert_eq!(l.pixels, originale.pixels, "pixel identici");
        assert_eq!(l.name, "gradiente");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_load_roundtrip_con_gruppo() {
        let path = temp("gruppo");
        let mut doc = Document::new(8, 8);
        doc.add_layer(layer("fondo", 8, 8, [10, 20, 30, 255]));
        let mut g = Group::new("gruppo");
        g.opacity = 0.75;
        g.blend = BlendMode::Multiply;
        g.add(Node::Layer(layer("a", 8, 8, [200, 10, 10, 255])));
        g.add(Node::Layer(layer("b", 8, 8, [10, 200, 10, 255])));
        doc.add_node(Node::Group(g));

        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");

        assert_eq!(letto.root.len(), 2);
        let gruppo = letto.root[1].as_group().expect("gruppo");
        assert_eq!(gruppo.name, "gruppo");
        assert!((gruppo.opacity - 0.75).abs() < 1e-6);
        assert_eq!(gruppo.blend, BlendMode::Multiply);
        assert_eq!(gruppo.children.len(), 2, "numero dei figli");
        assert_eq!(gruppo.children[0].name(), "a", "ordine preservato");
        assert_eq!(gruppo.children[1].name(), "b");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_load_roundtrip_con_mask() {
        let path = temp("mask");
        let mut doc = Document::new(24, 24);
        let mut l = layer("mascherato", 24, 24, [200, 150, 40, 255]);
        let mask = EllipseSelection::circle(12.0, 12.0, 8.0).to_mask(24, 24);
        l.mask = Some(std::sync::Arc::new(mask.clone()));
        doc.add_layer(l);

        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");

        let letta = letto.layers()[0].mask.as_ref().expect("mask");
        assert_eq!(letta.alpha, mask.alpha, "alpha identico");
        assert_eq!((letta.width, letta.height), (24, 24));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn la_maschera_condivisa_resta_condivisa() {
        let path = temp("mask-sharing");
        let mut doc = Document::new(8, 8);
        let mask = std::sync::Arc::new(EllipseSelection::circle(4.0, 4.0, 3.0).to_mask(8, 8));
        for i in 0..2 {
            let mut l = layer(&format!("l{i}"), 8, 8, [10 + i as u8, 20, 30, 255]);
            l.mask = Some(mask.clone());
            doc.add_layer(l);
        }

        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");
        let (a, b) = (
            letto.layers()[0].mask.clone().unwrap(),
            letto.layers()[1].mask.clone().unwrap(),
        );
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "la stessa maschera deve restare un solo Arc dopo il load"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_load_roundtrip_con_transform() {
        let path = temp("transform");
        let mut doc = Document::new(32, 32);
        let mut l = layer("spostato", 8, 8, [1, 2, 3, 255]);
        l.transform = Transform {
            x: 7.5,
            y: -3.25,
            scale_x: 2.0,
            scale_y: 0.5,
            rotation: 0.785,
        };
        doc.add_layer(l);

        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");
        let t = letto.layers()[0].transform;
        assert_eq!((t.x, t.y), (7.5, -3.25));
        assert_eq!((t.scale_x, t.scale_y), (2.0, 0.5));
        assert!((t.rotation - 0.785).abs() < 1e-6);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn uuid_stabile_tra_save_load_save() {
        let path1 = temp("uuid-1");
        let path2 = temp("uuid-2");
        let mut doc = Document::new(4, 4);
        doc.add_layer(layer("a", 4, 4, [1, 1, 1, 255]));
        let mut g = Group::new("g");
        g.add(Node::Layer(layer("b", 4, 4, [2, 2, 2, 255])));
        doc.add_node(Node::Group(g));

        save(&doc, &path1, &SaveOptions::default()).expect("save 1");
        let letto = load(&path1).expect("load");
        save(&letto, &path2, &SaveOptions::default()).expect("save 2");

        let m1 = peek_manifest(&path1).expect("manifest 1");
        let m2 = peek_manifest(&path2).expect("manifest 2");
        let mut u1: Vec<String> = m1.nodes.iter().map(|n| n.uuid.clone()).collect();
        let mut u2: Vec<String> = m2.nodes.iter().map(|n| n.uuid.clone()).collect();
        u1.sort();
        u2.sort();
        assert_eq!(u1, u2, "gli UUID non devono cambiare tra i salvataggi");
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }

    #[test]
    fn uuid_diversi_tra_documenti_diversi() {
        let path1 = temp("uuid-a");
        let path2 = temp("uuid-b");
        let mut a = Document::new(4, 4);
        a.add_layer(layer("a", 4, 4, [1, 1, 1, 255]));
        let mut b = Document::new(4, 4);
        b.add_layer(layer("b", 4, 4, [2, 2, 2, 255]));

        save(&a, &path1, &SaveOptions::default()).expect("save a");
        save(&b, &path2, &SaveOptions::default()).expect("save b");

        let ua = peek_manifest(&path1).unwrap().nodes[0].uuid.clone();
        let ub = peek_manifest(&path2).unwrap().nodes[0].uuid.clone();
        assert_ne!(ua, ub);
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }

    #[test]
    fn schema_version_futura_e_errore() {
        let path = temp("versione");
        let mut doc = Document::new(2, 2);
        doc.add_layer(layer("a", 2, 2, [1, 1, 1, 255]));
        save(&doc, &path, &SaveOptions::default()).expect("save");

        let mut m = peek_manifest(&path).expect("manifest");
        m.schema_version = 2;
        riscrivi_manifest(&path, &m);

        let err = errore(load(&path));
        assert_eq!(
            err,
            ProjectError::VersionMismatch {
                found: 2,
                max_supported: SCHEMA_VERSION
            }
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Riscrive il manifest dentro lo zip, lasciando gli asset.
    fn riscrivi_manifest(path: &Path, manifest: &Manifest) {
        let file = std::fs::File::open(path).expect("apri");
        let mut archivio = ZipArchive::new(file).expect("zip");
        let mut voci: Vec<(String, Vec<u8>)> = Vec::new();
        for i in 0..archivio.len() {
            let mut voce = archivio.by_index(i).expect("voce");
            let nome = voce.name().to_string();
            let mut dati = Vec::new();
            voce.read_to_end(&mut dati).expect("lettura");
            if nome != "manifest.json" {
                voci.push((nome, dati));
            }
        }
        let file = std::fs::File::create(path).expect("ricrea");
        let mut zip = ZipWriter::new(file);
        let opt = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("manifest.json", opt).expect("manifest");
        zip.write_all(serde_json::to_string(manifest).unwrap().as_bytes())
            .expect("scrittura");
        for (nome, dati) in voci {
            zip.start_file(nome, opt).expect("voce");
            zip.write_all(&dati).expect("dati");
        }
        zip.finish().expect("chiusura");
    }

    #[test]
    fn zstd_riduce_il_file() {
        let path_raw = temp("raw");
        let path_zstd = temp("zstd");
        let mut doc = Document::new(256, 256);
        doc.add_layer(gradient(256, 256, 0));
        doc.add_layer(gradient(256, 256, 37));

        save(
            &doc,
            &path_raw,
            &SaveOptions {
                codec: Codec::Raw,
                ..SaveOptions::default()
            },
        )
        .expect("save raw");
        save(&doc, &path_zstd, &SaveOptions::default()).expect("save zstd");

        let raw = std::fs::metadata(&path_raw).unwrap().len();
        let zstd = std::fs::metadata(&path_zstd).unwrap().len();
        let risparmio = 100.0 - (zstd as f64 / raw as f64 * 100.0);
        println!("raw {raw} byte, zstd {zstd} byte: -{risparmio:.1}%");
        assert!(
            risparmio >= 15.0,
            "zstd deve ridurre almeno del 15%: -{risparmio:.1}%"
        );
        let _ = std::fs::remove_file(&path_raw);
        let _ = std::fs::remove_file(&path_zstd);
    }

    #[test]
    fn file_corrotto_e_errore() {
        let path = temp("corrotto");
        let file = std::fs::File::create(&path).expect("crea");
        let mut zip = ZipWriter::new(file);
        let opt = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("assets/x.bin", opt).expect("voce");
        zip.write_all(b"spazzatura").expect("scrivi");
        zip.finish().expect("chiudi");

        let err = errore(load(&path));
        assert!(
            matches!(err, ProjectError::Manifest(_)),
            "atteso errore di manifest, trovato {err:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn il_composito_e_identico_dopo_il_roundtrip() {
        let path = temp("composito");
        let mut doc = Document::new(48, 40);
        doc.add_layer(gradient(48, 40, 11));
        let mut cerchio = layer("cerchio", 20, 20, [240, 240, 250, 200]);
        cerchio.transform = Transform::at(14.0, 9.0);
        doc.add_layer(cerchio);
        let mut gruppo = Group::new("gruppo");
        gruppo.opacity = 0.6;
        gruppo.add(Node::Layer(layer("velo", 48, 40, [0, 0, 0, 255])));
        doc.add_node(Node::Group(gruppo));

        let prima = doc.composite();
        save(&doc, &path, &SaveOptions::default()).expect("save");
        let letto = load(&path).expect("load");
        let dopo = letto.composite();

        assert_eq!(prima, dopo, "il composito deve essere identico byte per byte");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_grande_con_compressione_e_veloce() {
        let path = temp("grande");
        let mut doc = Document::new(1920, 1080);
        for i in 0..5 {
            doc.add_layer(gradient(1920, 1080, i * 20));
        }

        let fattore = if cfg!(debug_assertions) { 10 } else { 1 };
        let mut tempo_save = std::time::Duration::from_secs(999);
        let mut tempo_load = std::time::Duration::from_secs(999);
        for _ in 0..3 {
            let inizio = std::time::Instant::now();
            save(&doc, &path, &SaveOptions::default()).expect("save");
            tempo_save = tempo_save.min(inizio.elapsed());

            let inizio = std::time::Instant::now();
            let _ = load(&path).expect("load");
            tempo_load = tempo_load.min(inizio.elapsed());
        }

        let dimensione = std::fs::metadata(&path).unwrap().len();
        println!(
            "1920x1080 x5 layer: {} MB, save {tempo_save:?}, load {tempo_load:?}",
            dimensione / (1024 * 1024)
        );
        assert!(
            tempo_save.as_millis() < 2000 * fattore,
            "save in {tempo_save:?}: troppo lento"
        );
        assert!(
            tempo_load.as_millis() < 1000 * fattore,
            "load in {tempo_load:?}: troppo lento"
        );
        let _ = std::fs::remove_file(&path);
    }
}
