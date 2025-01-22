use dancey::Application;
use eframe::NativeOptions;

fn main() {
    let native_options = NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "Dancey",
        native_options,
        Box::new(|cc| Ok(Box::new(Application::new(cc)))),
    );
}
