use egui::{InnerResponse, Ui};

use crate::ui::panels::lib::Panel;

pub fn display_root(_this: &Panel, _ui: &mut Ui) -> anyhow::Result<Option<InnerResponse<()>>> {
    panic!()
}
