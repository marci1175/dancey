use egui::{CentralPanel, Context, Id, InnerResponse, RichText, Ui, ViewportId};

use crate::ui::panels::lib::Panel;

/// Display the media picker in the ui
pub fn display_media(this: &mut Panel, ctx: &Context) -> Option<InnerResponse<()>> {
    // Allocate the sidepanel for the panel
    // Match the detached panel's state
    match this.detached.load(std::sync::atomic::Ordering::Relaxed) {
        true => {
            // Clone panel state so that it can be cloned into a child window
            let this = this.clone();

            // Create child window for the detached panel
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of(this.id),
                this.viewport_settings.clone(),
                move |ctx, _viewport| {
                    // I am not sure why this is not working when creating the panel and the viewport
                    // If i uncomment this `ctx.request_repaint_of(ViewportId::ROOT);` wont work as intended
                    // ctx.send_viewport_cmd(egui::ViewportCommand::Title(String::from("Media")));
                    
                    CentralPanel::default().show(ctx, |ui| {
                        display_ui(&this, ctx, ui);
                    });
                },
            );

            None
        }
        false => Some({
            // Allocate sidepanel in the root ui
            egui::SidePanel::new(egui::panel::Side::Left, Id::new(this.id)).show(ctx, |ui| {
                // Display ui of the panel
                display_ui(this, ctx, ui);
            })
        }),
    }
}

/// This is what gets called when the panel is either attached or detached, the ui must conform to the current state of the panel
fn display_ui(this: &Panel, ctx: &Context, ui: &mut Ui) {
    if ui
        .button({
            match this.detached.load(std::sync::atomic::Ordering::Relaxed) {
                true => "Reattach",
                false => "Detach",
            }
        })
        .clicked()
    {
        // Perform a not operation on the current state
        this.detached
            .fetch_not(std::sync::atomic::Ordering::Relaxed);

        // Repaint root to close window
        ctx.request_repaint_of(ViewportId::ROOT);
    }
    
    ui.separator();

    
}
