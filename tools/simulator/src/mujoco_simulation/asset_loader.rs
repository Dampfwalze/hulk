use std::path::PathBuf;

use bevy::{
    asset::{
        AssetLoader, AssetPath, AsyncReadExt, LoadContext,
        io::{AssetSourceId, Reader},
    },
    platform::cell::SyncCell,
    prelude::*,
};
use mujoco_rs::{
    mujoco_c,
    prelude::*,
    wrappers::{
        MjVfs,
        mj_editing::{MjsHfield, MjsMesh, MjsSkin, MjsTexture, MjtBuiltin},
        traits::ModelTypeMut,
    },
};

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<MujocoSpec>()
        .init_asset_loader::<MjcfLoader>();
}

bevy::utils::define_atomic_id!(MujocoSpecId);

#[derive(Asset, TypePath)]
pub struct MujocoSpec {
    spec: SyncCell<MjSpecWrapper>,
    /// Needed for re-compiling the model when the spec is modified
    vfs: MjVfs,
    /// Bevy reader namespace; the XML directory lives in `spec.modelfiledir`.
    asset_source: AssetSourceId<'static>,

    instance_id: MujocoSpecId,
    revision: usize,
}

/// Wrapper around [MjSpec] that implements [Send]. [MjSpec] does not implement
/// [Send], because it uses shared references using reference counting internally.
///
/// # SAFETY
/// See [MjSpec::into_sendable]
struct MjSpecWrapper(MjSpec);

/// # SAFETY
/// See [MjSpec::into_sendable]
unsafe impl Send for MjSpecWrapper {}

impl MujocoSpec {
    pub fn empty() -> Self {
        Self {
            spec: SyncCell::new(MjSpecWrapper(MjSpec::new())),
            vfs: MjVfs::new(),
            asset_source: AssetSourceId::Default,
            instance_id: MujocoSpecId::new(),
            revision: 0,
        }
    }

    pub fn spec(&mut self) -> &MjSpec {
        &self.spec.get().0
    }

    pub fn spec_mut(&mut self) -> &mut MjSpec {
        self.revision += 1;
        &mut self.spec.get().0
    }

    pub fn revision(&self) -> usize {
        self.revision
    }

    pub fn instance_id(&self) -> MujocoSpecId {
        self.instance_id
    }

    pub(super) fn asset_source(&self) -> AssetSourceId<'static> {
        self.asset_source.clone()
    }

    pub fn compile(&mut self) -> Result<MjModel, MjEditError> {
        self.spec.get().0.compile_with_vfs(&self.vfs)
    }

    pub fn recompile<M: ModelTypeMut>(
        &mut self,
        mut data: MjData<M>,
    ) -> Result<MjData<M>, MjEditError> {
        let spec = unsafe { self.spec.get().0.ffi_mut() };

        let result = {
            let vfs = self.vfs.ffi();
            let model = unsafe { data.model_mut().ffi_mut() as *mut _ };
            let data = unsafe { data.ffi_mut() as *mut _ };
            unsafe { mujoco_c::mj_recompile(spec, vfs, model, data) }
        };

        if result != 0 {
            // data and model will be deleted if mj_recompile fails. This can be
            // a memory leak: The models container (like Box, Arc) will not be
            // dropped. The MjModel will be leaked, but its ffi pointer will be
            // invalid.
            std::mem::forget(data);

            let error = unsafe { mujoco_c::mjs_getError(spec) };
            let error = if error.is_null() {
                "Unknown error".to_owned()
            } else {
                unsafe { std::ffi::CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned()
            };
            Err(MjEditError::CompileFailed(error))
        } else {
            Ok(data)
        }
    }
}

impl MjSpecWrapper {
    /// # SAFETY
    /// See [MjSpec::into_sendable]
    pub unsafe fn new(spec: MjSpec) -> Self {
        MjSpecWrapper(spec)
    }
}

#[derive(TypePath, Default)]
pub struct MjcfLoader;

impl AssetLoader for MjcfLoader {
    type Asset = MujocoSpec;

    type Settings = ();

    type Error = BevyError;

    fn extensions(&self) -> &[&str] {
        &["xml"]
    }

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut spec_src = String::new();
        reader.read_to_string(&mut spec_src).await?;

        let mut vfs = MjVfs::new();
        vfs.add_from_buffer(load_context.path().path(), spec_src.as_bytes())?;
        let asset_source = load_context.path().source().clone_owned();

        let (deps, spec) = {
            let spec = MjSpec::from_xml_vfs(load_context.path().path(), &vfs)?; // <model> references should fail here, because VFS does not contain other files yet.

            let meshdir = spec.compiler().meshdir();
            let texturedir = spec.compiler().texturedir();
            let deps: Vec<AssetPath<'static>> = spec
                .mesh_iter()
                .map(MjsMesh::file)
                .chain(spec.hfield_iter().map(MjsHfield::file))
                .chain(spec.skin_iter().map(MjsSkin::file))
                .map(|file| (meshdir, file))
                .chain(
                    spec.texture_iter()
                        .filter(|texture| texture.builtin() == MjtBuiltin::mjBUILTIN_NONE)
                        .map(MjsTexture::file)
                        .map(|file| (texturedir, file)),
                )
                .filter(|(_, file)| !file.is_empty())
                .map(|(directory, file)| {
                    AssetPath::from(resolve_resource_path(&spec, directory, file))
                        .with_source(asset_source.clone())
                })
                .collect();

            (deps, spec)
        };

        // SAFETY: Never cloned yet (so <model> still allowed), and nothing attached yet
        let spec = unsafe { spec.into_sendable() };

        // Load all dependencies
        for dep in deps {
            info!("Loading dependency: {dep}");
            let dep_bytes = load_context.read_asset_bytes(dep.clone()).await?;

            // Use the exact compiler lookup path, preserving directories and case.
            let res = vfs.add_from_buffer(dep.path(), &dep_bytes);
            use mujoco_rs::error::MjVfsError::*;
            match res {
                Ok(_) => {}
                Err(AlreadyExists) => {
                    warn!("Dependency {dep} already exists in VFS, skipping");
                }
                Err(InvalidUtf8Path) => unreachable!("InvalidUtf8Path"),
                Err(NotFound) => unreachable!("NotFound"),
                Err(e) => {
                    return Err(format!("Failed to add dependency {dep} to VFS: {e:?}").into());
                }
            }
        }

        let spec = spec.take();

        Ok(MujocoSpec {
            // SAFETY: Never cloned yet (so <model> still allowed), and nothing attached yet
            spec: SyncCell::new(unsafe { MjSpecWrapper::new(spec) }),
            vfs,
            asset_source,
            instance_id: MujocoSpecId::new(),
            revision: 0,
        })
    }
}

/// MuJoCo combines compiler directories with filenames, then the model directory.
/// An absolute path overrides preceding directories; both slash styles are separators.
/// The parser has already applied `assetdir` defaults to the compiler directories.
///
/// (AI Slop)
pub(super) fn resolve_resource_path(spec: &MjSpec, directory: &str, file: &str) -> PathBuf {
    fn join(base: &str, path: &str) -> String {
        let path = path.replace('\\', "/");
        if base.is_empty() || path.starts_with('/') || path.contains(":/") {
            path
        } else {
            let base = base.replace('\\', "/");
            let separator = if base.ends_with('/') { "" } else { "/" };
            format!("{base}{separator}{path}")
        }
    }

    let file = if spec.strippath() {
        file.rsplit(['/', '\\']).next().unwrap_or(file)
    } else {
        file
    };
    let full_path = join(spec.modelfiledir(), &join(directory, file));
    // Match MuJoCo FilePath::PathReduce, including repeated separators, so VFS
    // lookups match exactly rather than falling back to an ambiguous basename.
    let prefix_len = if full_path.starts_with('/') {
        1
    } else {
        full_path.find(":/").map_or(0, |index| index + 2)
    };
    let (prefix, rest) = full_path.split_at(prefix_len);
    let mut components = Vec::new();
    let mut segments = rest.split('/');
    let filename = segments.next_back().unwrap_or("");
    for component in segments {
        match component {
            "." => {}
            ".." if components.last().is_some_and(|last| *last != "..") => {
                components.pop();
            }
            _ => components.push(component),
        }
    }
    components.push(filename);
    PathBuf::from(format!("{prefix}{}", components.join("/")))
}
