use bevy::prelude::*;

pub mod body;
pub mod geom;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((body::MujocoBody::plugin, geom::MujocoGeom::plugin));
}
