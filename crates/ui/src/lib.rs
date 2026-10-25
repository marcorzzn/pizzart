//! # pizzart-ui
//!
//! L'MVP dell'interfaccia: canvas, pannelli, strumenti. Serve a **scoprire i
//! buchi del motore** con l'uso reale, non a essere bella.
//!
//! La logica sta in [`state::Editor`], che non conosce egui: i comportamenti che
//! si possono sbagliare (drag dello slider che inonda la history, selezione che
//! diventa maschera, undo che deve invalidare il canvas) sono testabili senza
//! aprire una finestra.

#![forbid(unsafe_code)]

pub mod app;
pub mod canvas;
pub mod dialogs;
pub mod panels;
pub mod state;
pub mod tools;

pub use app::PizzartApp;
pub use state::{Editor, Tool};
