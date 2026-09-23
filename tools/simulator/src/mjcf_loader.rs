use std::convert::Infallible;

use bevy::{
    asset::{AssetLoader, AsyncReadExt, LoadContext, io::Reader},
    prelude::*,
};
use mujoco_rs::wrappers::{
    MjSpec, MjVfs, SpecItem,
    mj_editing::{MjsHfield, MjsMesh, MjsSkin, MjsTexture},
};

pub fn plugin(app: &mut App) {
    app.init_asset::<MyAsset>()
        .preregister_asset_loader::<MjcfLoader>(&["xml"])
        .init_asset_loader::<MjcfLoader>();
}

#[derive(TypePath, Default)]
pub struct MjcfLoader;

#[derive(Asset, TypePath, Default)]
pub struct MyAsset;

impl AssetLoader for MjcfLoader {
    type Asset = MyAsset;

    type Settings = ();

    type Error = Infallible;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut spec_src = String::new();
        reader
            .read_to_string(&mut spec_src)
            .await
            .expect("TODO: Handle IO error");

        let mut vfs = MjVfs::new();
        vfs.add_from_buffer(load_context.path().path(), spec_src.as_bytes())
            .expect("Should not error");

        let (deps, spec) = {
            let spec = MjSpec::from_xml_vfs(load_context.path().path(), &vfs)
                .expect("TODO: Handle parse error"); // <model> references should fail here, because VFS does not contain other files yet.

            let mut deps = Vec::<String>::new();

            deps.extend(spec.mesh_iter().map(MjsMesh::file).map(ToOwned::to_owned));
            deps.extend(
                spec.hfield_iter()
                    .map(MjsHfield::file)
                    .map(ToOwned::to_owned),
            );
            deps.extend(spec.skin_iter().map(MjsSkin::file).map(ToOwned::to_owned));
            deps.extend(
                spec.texture_iter()
                    .map(MjsTexture::file)
                    .map(ToOwned::to_owned),
            );

            (deps, spec)
        };

        // SAFETY: Never cloned yet (so <model> still allowed), and nothing attached yet
        let spec = unsafe { spec.into_sendable() };

        // let mut lc = load_context.begin_labeled_asset();

        for dep in deps.iter() {
            info!("Dependency: {dep}");
        }

        // Load all dependencies
        for dep in deps {
            info!("Loading dependency: {dep}");
            let dep_bytes = load_context.read_asset_bytes(&dep).await.unwrap();
            info!("Dependency {dep} loaded, adding to VFS");

            let res = vfs.add_from_buffer(&dep, &dep_bytes);
            use mujoco_rs::error::MjVfsError::*;
            match res {
                Ok(_) => {}
                Err(AlreadyExists) => {
                    warn!("Dependency {dep} already exists in VFS, skipping");
                }
                Err(InvalidUtf8Path) => unreachable!("InvalidUtf8Path"),
                Err(NotFound) => unreachable!("NotFound"),
                Err(e) => {
                    panic!("Failed to add dependency {dep} to VFS: {e:?}");
                    // return Err(e.into());
                }
            }
        }

        let mut spec = spec.take();

        for body in spec.body_iter() {
            info!("Body: {}", body.name());
        }

        let model = spec
            .compile_with_vfs(&vfs)
            .expect("TODO: Handle compile error");

        // load_context.add_loaded_labeled_asset("Spec", lc.finish(spec));

        // let mut model = spec.compile().expect("TODO: Handle compile error");
        // let x = model.mesh("name").unwrap().view_mut(&mut model).;

        Ok(MyAsset)
    }
}

#[derive(Asset, TypePath)]
pub struct StlSrc(Box<[u8]>);

#[derive(TypePath, Default)]
pub struct StlLoader;
