use bevy::prelude::*;

pub mod asset_loader;
pub mod assets;
pub mod components;
mod conversion;
pub mod identification;
pub mod simulation_step;
pub mod simulation_sync;
pub mod world_items;

#[cfg(test)]
mod playground;

pub use asset_loader::MujocoSpec;
pub use components::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        asset_loader::plugin,
        identification::plugin,
        components::plugin,
        simulation_sync::plugin,
        simulation_step::plugin,
        assets::plugin,
        world_items::plugin,
    ));
}
