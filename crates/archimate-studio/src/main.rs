//! ArchiMate Studio: a desktop ArchiMate modelling tool that reads and writes Archi models.

mod app;
mod canvas;
mod notation;

fn main() -> eframe::Result {
    let file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("ArchiMate Studio")
            .with_inner_size([1400.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ArchiMate Studio",
        options,
        Box::new(move |_cc| Ok(Box::new(app::Studio::new(file)))),
    )
}
