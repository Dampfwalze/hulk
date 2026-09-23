use bevy::{
    math::{DQuat, DVec3},
    prelude::*,
};
use mujoco_rs::wrappers::{MjData, mj_editing::MjsBody, traits::ModelType};

use crate::mujoco_simulation::{
    MujocoSimulationData,
    conversion::MujocoToBevy,
    identification::{MujocoItemId, MujocoItemName, MujocoItemOf, Required},
    simulation_sync::MujocoSyncData,
};

#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(MujocoItemName<MjsBody> = Required, Transform, Visibility)]
#[component(immutable)]
pub struct MujocoBody;

impl MujocoBody {
    pub(super) fn plugin(app: &mut App) {
        app.register_type::<Self>().add_observer(Self::extract_body);
    }

    fn extract_body(
        e: On<MujocoSyncData<MjsBody>>,
        body: Query<(&MujocoItemId<MjsBody>, &MujocoItemOf)>,
        simulation: Query<&MujocoSimulationData>,
        mut commands: Commands,
    ) -> Result {
        let (id, &MujocoItemOf(sim_entity)) = body.get(e.entity())?;
        let id = **id;

        let data = simulation.get(sim_entity)?;

        let (position, rotation) = Self::relative_transform(data, id);

        commands.entity(e.entity()).insert((
            MujocoBody,
            Transform {
                translation: position.as_vec3(),
                rotation: rotation.as_quat(),
                scale: Vec3::ONE,
            },
        ));

        Ok(())
    }

    fn relative_transform<M: ModelType>(data: &MjData<M>, id: usize) -> (DVec3, DQuat) {
        let position: DVec3 = data.xpos()[id].to_bevy();
        let rotation: DQuat = data.xquat()[id].to_bevy();
        let parent_id = data.model().body_parentid()[id] as usize;
        let parent_position: DVec3 = data.xpos()[parent_id].to_bevy();
        let parent_rotation: DQuat = data.xquat()[parent_id].to_bevy();
        let parent_inverse = parent_rotation.inverse();

        (
            (parent_inverse * (position - parent_position)),
            (parent_inverse * rotation),
        )
    }
}
