use egui::Ui;

pub mod settings;
pub mod plugins;

macro_rules! create_window_states {
    ($visibility:vis, $($window_name:ident => { $($state_field:ident : $state_ty:ty),* }),*) => {
        paste::paste! {
            #[derive(Default, Debug)]
            $visibility struct WindowsManager {
                $(
                    $visibility [<$window_name:lower>]: bool,
                )*
            }
        }

        $(
            paste::paste! {
                $visibility struct [<$window_name State>] {
                    $(
                        $visibility $state_field: $state_ty,
                    )*
                }
            }
        )*
    };
}

create_window_states! (pub, Settings => {  }, Plugins => {  }, Help => {  });

impl WindowsManager {
    pub fn display_windows(&self, ui: &mut Ui) {
        
    }
}

