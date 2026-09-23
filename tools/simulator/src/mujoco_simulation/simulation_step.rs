use bevy::prelude::*;

use super::components::*;

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, step_simulations);
}

fn step_simulations(mut simulations: Query<&mut MujocoSimulationData>) {
    for mut simulation in simulations.iter_mut() {
        simulation.step();
    }
}
