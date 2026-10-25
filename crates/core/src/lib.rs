//! # pizzart-core
//!
//! Il cuore dell'editor: documento, layer, trasformazioni, fusione, undo/redo.
//!
//! ## Principi
//!
//! - **Nessuna GPU, nessuna dipendenza esterna.** Il core si compila e si testa
//!   ovunque, anche offline: la GPU è un dettaglio del crate `pizzart-render`.
//! - **Funzioni pure dove conta.** Le formule di fusione sono funzioni testabili
//!   e diventano il riferimento per la versione GPU, che deve produrre gli stessi
//!   valori.
//! - **Coordinate esplicite.** Un layer vive nei propri pixel; `Transform` lo
//!   porta nello spazio del documento e poi in NDC per lo shader.
//!
//! ## Esempio
//!
//! ```
//! use pizzart_core::{AddLayer, BlendMode, Document, Layer, SetOpacity};
//!
//! let mut doc = Document::new(64, 64);
//! let sfondo = Layer::filled("sfondo", 64, 64, [20, 20, 30, 255]);
//! doc.execute(Box::new(AddLayer::new(sfondo)));
//!
//! let velo = Layer::filled("velo", 64, 64, [255, 255, 255, 255]);
//! let id = velo.id;
//! doc.execute(Box::new(AddLayer::new(velo)));
//! doc.execute(Box::new(SetOpacity::new(id, 0.5)));
//!
//! let rgba = doc.composite();
//! assert_eq!(rgba.len(), 64 * 64 * 4);
//!
//! doc.undo();
//! assert_eq!(doc.layer(id).unwrap().opacity, 1.0);
//!
//! let _ = BlendMode::Multiply.label_it();
//! ```

#![forbid(unsafe_code)]

pub mod blend;
pub mod compositor;
pub mod document;
pub mod error;
pub mod history;
pub mod image;
pub mod layer;
pub mod mask;
pub mod node;
pub mod selection;
pub mod stable;
pub mod math;
pub mod transform;

pub use blend::BlendMode;
pub use compositor::composite;
pub use document::Document;
pub use error::LayerError;
pub use node::{Group, InsertWhere, Node};
pub use history::{
    AddLayer, Command, History, RemoveLayer, RenameLayer, SetBlendMode, SetLayerMask, SetOpacity, SetVisible,
    SetTransform, ToggleMaskInverted,
};
pub use image::PlacedImage;
pub use layer::{Layer, LayerId};
pub use mask::{Mask, MaskSource, SharedMask};
pub use selection::{EllipseSelection, LassoSelection, RectSelection};
pub use stable::StableId;
pub use math::Mat4;
pub use transform::Transform;
