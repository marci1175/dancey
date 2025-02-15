use dancey::app::Application;
use eframe::NativeOptions;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let native_options = NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "Dancey",
        native_options,
        Box::new(|cc| Ok(Box::new(Application::new(cc)))),
    )
}
