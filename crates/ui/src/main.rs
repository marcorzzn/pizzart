//! Pizzart — editor grafico.
//!
//! ```bash
//! cargo run --offline --release -p pizzart-ui
//! ```

use pizzart_ui::PizzartApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Pizzart"),
        ..Default::default()
    };
    eframe::run_native(
        "Pizzart",
        options,
        Box::new(|cc| Ok(Box::new(PizzartApp::new(cc)))),
    )
}
