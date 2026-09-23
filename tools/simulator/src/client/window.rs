use bevy::prelude::*;

#[derive(Default)]
pub struct ClientWindowPlugin {
    create_first_window: bool,
}

impl Plugin for ClientWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::winit::WinitPlugin::default());
    }
}

#[derive(Component)]
pub struct ClientWindow;
