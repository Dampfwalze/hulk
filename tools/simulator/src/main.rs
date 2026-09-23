use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    prelude::*,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use mujoco_rs::wrappers::{MjtGeom, SpecItem};

mod client;
mod mesh_loaders;
mod mjcf_loader;
mod mujoco_simulation;

fn main() {
    let mut app = App::new();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let asset_path = std::path::Path::new(manifest_dir).join("assets");

    app.add_plugins((
        DefaultPlugins
            // Don't spawn a window. WindowPlugin must not be disabled, because
            // bevy renderer depends on it.
            .set(WindowPlugin {
                // primary_window: None,
                // exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                // Erzwingt, dass Bevy immer diesen absoluten Pfad nutzt,
                // egal von wo aus das Programm gestartet wurde
                file_path: asset_path.to_string_lossy().to_string(),
                // MuJoCo resource loads explicitly opt into absolute file paths.
                unapproved_path_mode: bevy::asset::UnapprovedPathMode::Deny,
                ..default()
            }),
        //
        // WinitPlugin will panic when there is no window environment
        // available, so disable it by default. When using client plugin, it
        // will be re-enabled there.
        // .disable::<bevy::winit::WinitPlugin>()
        //
        // Needed when WinitPlugin is disabled. When WinitPlugin is
        // re-enabled, it will override the app runner with its event
        // loop.
        // .add(bevy::app::ScheduleRunnerPlugin::default()),
        EguiPlugin::default(),
        WorldInspectorPlugin::new(),
        FreeCameraPlugin,
    ))
    .add_plugins((
        // mjcf_loader::plugin,
        mujoco_simulation::plugin,
        mesh_loaders::plugin,
    ))
    .add_systems(Startup, test_setup);

    app.run();
}

fn test_setup(
    assets: Res<AssetServer>,
    mut commands: Commands,
    mut spec_assets: ResMut<Assets<mujoco_simulation::MujocoSpec>>,
) {
    // let mut spec = mujoco_simulation::MujocoSpec::empty();
    // spec.spec_mut()
    //     .world_body_mut()
    //     .add_body()
    //     .with_name("ground")
    //     .add_geom()
    //     .with_type(MjtGeom::mjGEOM_PLANE)
    //     .with_size([10., 10., 0.1]);

    // let def_spec_handle: Handle<mujoco_simulation::MujocoSpec> = spec_assets.add(spec);

    let spec_handle: Handle<mujoco_simulation::MujocoSpec> = assets.load("k1_robot.xml");

    commands.spawn((
        Name::new("Mjcf World"),
        mujoco_simulation::MujocoSimulation(spec_handle),
        // Make local Z axis up
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera {
            // sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 2.0,
            run_speed: 9.0,
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight { ..default() },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
