use bevy::prelude::*;

pub fn plugin(app: &mut App) {}

#[derive(Component)]
#[require(
    Camera3d,
    Transform = Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y)
)]
pub struct SimulationObserver;
