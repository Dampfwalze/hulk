use bevy::prelude::*;

mod client;
mod mjcf_loader;
mod mujoco_spec;

fn main() {
    std::thread::Builder::new()
        .name("async-io-driver".into())
        .spawn(|| {
            loop {
                // This forces async-io to process pending OS readiness notifications
                async_io::block_on(async {
                    async_io::Timer::after(std::time::Duration::from_millis(5)).await;
                });
            }
        })
        .unwrap();

    let mut app = App::new();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let asset_path = std::path::Path::new(manifest_dir).join("assets");

    app.add_plugins(
        (
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
                    ..default()
                })
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
            // mjcf_loader::plugin,
        ),
    )
    .add_plugins(mjcf_loader::plugin)
    .add_systems(Startup, test_setup);

    app.run();
}

fn test_setup(assets: Res<AssetServer>, mut commands: Commands) {
    let spec_handle: Handle<mjcf_loader::MyAsset> = assets.load("k1_robot.xml");
    // commands.spawn((
    //     spec_handle,
    //     Transform::from_xyz(0.0, 0.0, 0.0),
    //     GlobalTransform::default(),
    // ));
}
