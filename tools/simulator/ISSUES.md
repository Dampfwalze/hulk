# MuJoCo → Bevy visual bindings: running issue list

Review baseline: current simulator implementation, `mujoco-rs 6.1.0+mj-3.12.0`
and Bevy 0.19.1. Locations below refer to that review and may move as code changes.

Priorities and dispositions follow the application owner's decisions. **TODO**
entries include intentional omissions that are not currently needed by the
application; they are not requirements to implement immediately. **Next Steps**
contains planned runtime work. A checked **Non-Issue** records a closed review
item, rather than an implemented fix.

Keep issue IDs when moving entries between sections. Check an action item when
its completion criteria are met. Open this file in VS Code's Markdown Preview to
expand or collapse each entry's details; edit `[ ]` to `[x]` in the source to
record completion.

## Very-important

- [ ] **SIM-001 — Quaternion components are in the wrong order.**

<details>
<summary>Details and completion criteria</summary>

**Locations:** [body.rs](src/mujoco_simulation/world_items/body.rs),
[geom.rs](src/mujoco_simulation/world_items/geom.rs),
[mesh.rs](src/mujoco_simulation/assets/mesh.rs).

MuJoCo quaternions use `[w, x, y, z]`; Bevy/glam array constructors expect
`[x, y, z, w]`. Passing the array unchanged turns MuJoCo's identity into an X-axis
half-turn. This affects body poses, geom poses, and mesh metadata.

**Complete when:** every quaternion crossing the boundary uses an explicit,
consistent conversion. Keep component ordering separate from the simulation
root's coordinate-basis rotation.

[MuJoCo orientation convention](https://mujoco.readthedocs.io/en/3.12.0/modeling.html#frame-orientations).

</details>

- [ ] **SIM-002 — World-space body poses are assigned as parent-local transforms.**

<details>
<summary>Details and completion criteria</summary>

**Locations:** [body.rs](src/mujoco_simulation/world_items/body.rs),
[simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

`xpos` and `xquat` describe body poses in the MuJoCo world, while Bevy's
`Transform` is relative to `ChildOf`. Assigning world poses to the nested body
hierarchy applies ancestor transforms again.

**Complete when:** the representation consistently uses either current
parent-relative body poses or world-pose-driven bodies directly beneath the
simulation root. Do not substitute reference-model body poses for current
joint-dependent poses.

Unity writes these values to its world `position`/`rotation` properties:
[Unity body synchronization](https://github.com/google-deepmind/mujoco/blob/3.12.0/unity/Runtime/Components/Bodies/MjBody.cs#L38).

</details>

- [ ] **SIM-003 — `geom_size` is treated as a generic XYZ scale.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

`geom_size` has shape-specific meaning. Spheres use a radius; capsules and
cylinders use radius/half-length; boxes use half-extents; ellipsoids use radii.
Planes have extent/grid semantics. Mesh and heightfield geometry comes from
their assets. Copying every size triple into `Transform.scale` distorts meshes
and can introduce zero scale axes for primitives.

**Complete when:** dimensions are represented by the applicable shape variant,
independently of its pose. Compiled mesh vertices do not receive `geom_size` as
an additional scale.

[Geom size definitions](https://mujoco.readthedocs.io/en/3.12.0/XMLreference.html#body-geom).

</details>

- [ ] **SIM-004 — Compiler-centering metadata is mistaken for XML reference metadata.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [mesh.rs](src/mujoco_simulation/assets/mesh.rs).

Compiled `mesh_pos`/`mesh_quat` are stored as `ref_pos`/`ref_quat`, although they
describe the subsequent compiler centering/alignment. XML reference transforms
and scaling precede that operation. Compiled vertices already include the full
processing sequence; applying this metadata again transforms them twice. Raw
file vertices require a different sequence from the one currently applied.

**Complete when:** source reference transforms and compiled centering transforms
cannot be confused, and each mesh is transformed exactly once. Consuming compiled
vertices directly is the preferred visual contract; the file-backed extraction
change itself remains deferred under SIM-015.

[Compiler transformation sequence](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/user/user_mesh.cc#L1314-L1391).

</details>

- [ ] **SIM-005 — Material indexing is never registered.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [identification.rs](src/mujoco_simulation/identification.rs).

Index hooks are registered for meshes, bodies, and geoms, but not materials.
Geom extraction nevertheless looks up material entities by compiled ID. These
lookups fail; later hierarchy synchronization can create duplicate untracked
material entities.

**Complete when:** the material category participates in lookup and lifecycle
reconciliation consistently. This concerns asset lookup, not rendering asset
entities; see the closed design decision SIM-030.

</details>

- [ ] **SIM-006 — Recompilation publishes poses before recomputing them.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

Initial compilation calls `forward()` before extraction; recompilation does
not. MuJoCo 3.12 restores integration state during `mj_recompile` without
subsequently refreshing derived poses. Immediate body extraction can therefore
read invalid or reference-derived `xpos`/`xquat`.

**Complete when:** the required derived data is refreshed after state restoration
and before any synchronization event publishes it.

[MuJoCo recompile implementation](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/user/user_api.cc#L307).

</details>

- [ ] **SIM-007 — Replacing identity components leaves old index entries.**

<details>
<summary>Hook behavior, investigation, and completion criteria</summary>

**Location:** [identification.rs](src/mujoco_simulation/identification.rs).

**Owner's question:** "I did my research, I thought `on_remove` would cover replacement?"

**Verified for Bevy 0.19.1:** `on_remove` covers removal without replacement,
including despawning. Replacing an existing component through insertion runs
`on_discard` for the old value and `on_insert` for the new value. It does not run
`on_remove`. `on_discard` also runs for removal. These hooks do not cover ordinary
in-place mutation; the existing immutable components avoid that particular gap.

The current insert/remove pairing can add a new name/ID key without deleting
its old key. Changing `MujocoItemOf` also needs an explicit index policy.

**Complete when:** replacement removes the previous binding before adding its
replacement, and ownership changes cannot leave entries in the old simulation.
Use `on_discard` with `on_insert`, or rebuild the indexes transactionally. Cleanup
should remove a key only if it still points to the entity being cleaned up.

[Bevy 0.19.1 lifecycle contract](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_ecs/src/lifecycle.rs#L15-L39).

</details>

## Important

- [ ] **SIM-008 — An ordinary mesh without UVs causes an invalid slice.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [mesh.rs](src/mujoco_simulation/assets/mesh.rs).

Absent UV data has `mesh_texcoordadr = -1` and count zero. Casting the address to
`usize` and slicing before testing absence panics. Checking whether the resulting
vector is empty happens too late. These values were confirmed with the local
MuJoCo 3.12 library.

**Complete when:** the optional range is decoded before slicing, and a mesh with
no UVs produces `None` without indexing the UV buffer.

[Compiler UV sentinel handling](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/user/user_model.cc#L3236-L3259).

</details>

- [ ] **SIM-009 — Retained entities keep obsolete parents.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

`ChildOf` is assigned only while spawning. Moving a retained body or geom leaves
it beneath its previous parent. Removing that parent can recursively despawn an
entity that reconciliation intended to retain.

**Complete when:** parent relationships are reconciled on structural changes,
before obsolete parents are removed.

</details>

- [ ] **SIM-010 — Reloaded or different specs can be mistaken for the existing spec.**

<details>
<summary>Details and completion criteria</summary>

**Locations:** [simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs),
[asset_loader.rs](src/mujoco_simulation/asset_loader.rs).

Recompilation is skipped solely by comparing revision integers. Every XML load
starts at revision zero. Reloading or replacing an asset, or switching to a
different asset with the same revision, can retain the previous model/visuals.

**Complete when:** invalidation accounts for asset identity and replacement
generation, rather than only the asset-local revision number.

</details>

- [ ] **SIM-011 — Fileless assets are treated as external files.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [asset_loader.rs](src/mujoco_simulation/asset_loader.rs).

Dependency collection includes empty `file()` values and attempts
`read_asset_bytes("")`. Valid inline meshes, inline heightfields, and procedural
textures can fail before reaching compilation.

**Complete when:** only actual, nonempty external resource references are loaded;
fileless assets reach compilation normally.

</details>

- [ ] **SIM-012 — Resource paths use the wrong resolution rules.**

<details>
<summary>Details and completion criteria</summary>

**Locations:** [asset_loader.rs](src/mujoco_simulation/asset_loader.rs),
[mesh.rs](src/mujoco_simulation/assets/mesh.rs).

Raw filenames are interpreted relative to Bevy's asset root without applying
the main XML directory and MuJoCo compiler asset-directory rules. The independent
file-backed rendering path repeats the lookup.

**Complete when:** supported resources are resolved consistently with MuJoCo and
both sides use the same content. This does not require enabling external model
references; their support policy is tracked under SIM-026.

[MuJoCo path resolution](https://mujoco.readthedocs.io/en/3.12.0/XMLreference.html#compiler-meshdir).

</details>

- [ ] **SIM-013 — The mesh type permits states that do not describe compiled meshes.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [mesh.rs](src/mujoco_simulation/assets/mesh.rs).

Normal values and normal indices are independent options, as are UV values and
UV indices. MuJoCo generates missing normals for compiled meshes and supplies
their corner indices. An absent UV set still has placeholder entries in the
global UV-index buffer, so that buffer's nonemptiness does not establish UV
presence.

**Complete when:** the intermediate type carries compiled invariants: required
normals with corresponding triangle indices, plus an optional UV set containing
both coordinates and triangle indices. Keep position, normal, and UV indices in
their independent domains.

[Compiled normal handling](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/user/user_mesh.cc#L1271-L1290).

</details>

- [ ] **SIM-014 — Material approximation policy is implicit.**

<details>
<summary>Details and completion criteria</summary>

**Location:** [material.rs](src/mujoco_simulation/assets/material.rs).

MuJoCo `reflectance` is extracted but unused; legacy `specular` is mapped to
Bevy's differently defined reflectance parameter, and legacy shininess is
approximated as PBR roughness. This conversion cannot promise identical shading.

**Complete when:** compiled material semantics remain clear in the intermediate
type and the renderer's approximation policy is explicit, including which
properties are unsupported. Exact visual parity is not required by this item.

[MuJoCo material semantics](https://mujoco.readthedocs.io/en/3.12.0/XMLreference.html#asset-material).

</details>

## TODO

- [ ] **SIM-015 — File-backed meshes bypass MuJoCo's compiled visual geometry.**

<details>
<summary>Deferred work</summary>

**Locations:** [mesh.rs](src/mujoco_simulation/assets/mesh.rs),
[mesh_loaders.rs](src/mesh_loaders.rs).

File-backed assets are decoded again by Bevy, while inline assets use compiled
arrays. The STL-only decoder ignores `content_type`, uses file normals rather
than MuJoCo's generated normals, and does not reproduce compiler smoothing or
negative-scale winding correction.

**Future completion:** use compiled position, normal, UV, and corner-index arrays
for file-backed meshes too, keeping filenames as optional provenance. Coordinate
contract correctness is separately prioritized under SIM-004.

</details>

- [ ] **SIM-016 — Different mesh definitions can share the first definition's Bevy asset.**

<details>
<summary>Deferred work</summary>

**Location:** [mesh.rs](src/mujoco_simulation/assets/mesh.rs).

Different MuJoCo mesh definitions can use one filename with different scale or
reference settings. Bevy's path/type cache does not distinguish those settings;
the first loaded geometry can therefore be reused. Later settings changes can
also reuse the existing asset.

**Future completion:** each distinct compiled mesh has the appropriate Bevy asset
identity. Building assets from compiled arrays can resolve this alongside SIM-015.

[Bevy asset cache](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_asset/src/server/info.rs).

</details>

- [ ] **SIM-017 — Most geom variants never produce geometry.**

<details>
<summary>Intentionally deferred implementation</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

Primitive, heightfield, and SDF builders currently log a TODO. Implement only
the shapes needed by the application. A mesh-to-unsupported-shape transition
also needs a deliberate policy so the old `Mesh3d` does not silently remain.

**Future completion:** supported shapes render correctly and shape transitions
reconcile render components. Explicitly unsupported shapes remain documented.

</details>

- [ ] **SIM-018 — Heightfield references cannot resolve.**

<details>
<summary>Deferred work</summary>

**Locations:** [geom.rs](src/mujoco_simulation/world_items/geom.rs),
[simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

Geom extraction expects an indexed heightfield asset, but no such entity is
created or extracted.

**Future completion, if needed:** extract grid dimensions, elevation samples,
and the four heightfield size parameters; index and render the resulting asset.

</details>

- [ ] **SIM-019 — Geom color overrides are discarded when a material exists.**

<details>
<summary>Deferred work</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

The material-backed path ignores the geom's RGBA. MuJoCo applies a nondefault
geom RGBA in preference to material RGBA while retaining the remaining material
properties.

**Future completion:** represent and resolve effective appearance, including
color/opacity overrides, without unintentionally modifying other users of a
shared base material.

[MuJoCo appearance resolution](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/engine/engine_vis_visualize.c#L208-L230).

</details>

- [ ] **SIM-020 — The SDF variant discards its visual mesh reference.**

<details>
<summary>Deferred work</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

The payload-free `Sdf` variant loses the associated mesh referenced by
`geom_dataid`. That mesh is sufficient for visual rendering without passing SDF
collision behavior into Bevy.

**Future completion, if needed:** retain the mesh reference, potentially sharing
the visual mesh variant used for ordinary mesh geoms.

</details>

- [ ] **SIM-021 — Standalone RGBA opacity is not applied.**

<details>
<summary>Deferred work</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

The standalone RGBA branch sets alpha but leaves `AlphaMode::Opaque`. Transparent
mesh geoms can render opaque; alpha-zero collision primitives will have the same
problem once their rendering is implemented.

**Future completion:** apply a consistent opacity policy to effective appearance,
including fully invisible geoms.

</details>

- [ ] **SIM-022 — Texture data is dropped completely.**

<details>
<summary>Wrapper investigation and deferred work</summary>

**Location:** [material.rs](src/mujoco_simulation/assets/material.rs).

**Owner's observation:** referenced textures were not found in `mujoco_rs`.

**Verified in the installed wrapper:** `MjsMaterial` exposes texture assignment
methods, but its generated texture-vector API has no corresponding getter. The
compiled model does expose references: `model.mat_texid()[material_id][role]`,
or a material view's `texid[role]`. A value of `-1` means no texture. The wrapper
also exposes compiled texture pixels and metadata through `tex_data()`,
`tex_adr()`, `tex_width()`, `tex_height()`, `tex_nchannel()`, `tex_type()`, and
`tex_colorspace()`.

The current intermediate material always has an empty texture list, and mapping
parameters do not reach rendering. This remains intentionally deferred; the
compiled-model API provides a route when needed.

**Future completion:** retain texture references by role, extract compiled
texture assets, and apply supported mapping/repetition behavior.

[Wrapper source at the installed package's recorded commit](https://github.com/davidhozic/mujoco-rs/blob/b65f0274d773299f957c603a48da43db41805a0b/src/wrappers/mj_model.rs).

</details>

- [ ] **SIM-023 — Geom visibility groups are lost.**

<details>
<summary>Deferred work</summary>

**Location:** [geom.rs](src/mujoco_simulation/world_items/geom.rs).

`geom_group` is not retained, preventing group-based display choices. Collision
flags alone are not a substitute for visual visibility policy.

**Future completion, if needed:** retain the group tag and let the viewer decide
which groups to display.

</details>

- [ ] **SIM-024 — Optional names are used as unique identities; retained compiled IDs become obsolete.**

<details>
<summary>Intended identity design and deferred reconciliation</summary>

**Locations:** [identification.rs](src/mujoco_simulation/identification.rs),
[simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

**Intentional deferral:** dual identification adds complexity. Entities for named
items should follow those items' lifetime. ID-based identification does not need
the same continuity guarantee. The detailed design will be discussed separately.

**Current consequences:** unnamed objects share the empty-string name key,
overwriting lookup and cleanup inventory entries. Later reconciliation can
collapse them onto one entity. Retained entities also keep IDs assigned only at
spawn; compilation can reassign those IDs after insertion/deletion/reordering.

This entry includes the original finding **"Retained entities keep obsolete
compiled IDs."** It is not a separate immediate task.

**Future completion:** implement the intended named-item lifetime behavior and a
separate ID-based policy; refresh compiled bindings and maintain a complete
entity inventory without requiring persistent identities for unnamed items.
Coordinate index replacement behavior with SIM-007.

</details>

- [ ] **SIM-025 — Failed recompilation leaves an apparently valid visual scene.**

<details>
<summary>Desired failure behavior and deferred work</summary>

**Location:** [simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

**Desired behavior:** keep the last successfully displayed scene on compilation
failure. Loading new or changed specs is an expected user action; compilation
errors must be handled properly.

The outstanding issue is the missing distinction between that retained scene and
the requested spec that failed compilation. Removing simulation data alone does
not communicate that distinction.

**Future completion:** clearly mark the retained scene as the last successful
result, expose the compilation error and failed load/update status, and allow
recovery after a corrected spec is supplied. Do not silently imply that the
retained scene represents the failed spec. Removing the scene is not required.

</details>

- [ ] **SIM-026 — Dependency discovery is incomplete and too late for parsing.**

<details>
<summary>Support-policy decision and deferred work</summary>

**Location:** [asset_loader.rs](src/mujoco_simulation/asset_loader.rs).

Parsing currently starts with only the main XML in the VFS. Parse-time resources
such as included XML or referenced models are not provided first. Dependency
collection also omits separately specified cubemap face files.

**Owner's constraints:** referenced models are not currently needed and raise
race-condition concerns. Disallowing them is under consideration. Avoid adding
an independent XML pre-parser merely to discover dependencies.

**Future completion:** choose and document the supported resource subset. Reject
unsupported references with a clear diagnostic, or provide resource resolution
for supported cases without assuming a separate XML parsing implementation is
required. Treat included XML, external model references, and cubemap resources
as distinct support decisions. Full referenced-model support is not required by
this entry.

</details>

- [ ] **SIM-027 — The scene representation covers only a subset of visual objects.**

<details>
<summary>Intentionally limited scope</summary>

**Locations:** [world_items/mod.rs](src/mujoco_simulation/world_items/mod.rs),
[simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs).

Current extraction covers bodies/geoms and mesh/material assets. The following
remain unsupported and should be added only if the application needs them:

| Category | Missing visual information |
| --- | --- |
| Skins and flexes | Deformable surfaces and their changing geometry. |
| Model lights | Model-defined illumination, including computed light poses. |
| Model cameras | Camera poses and projection parameters. |
| Sites | Visual markers. |
| Spatial tendons | Computed visible cable paths. |
| Global visual settings | Selected environment, headlight, clipping, and viewer settings. |

**Future completion:** explicitly document supported categories and implement any
that become necessary. This is a scope inventory, not a requirement for full
MuJoCo viewer parity or transfer of nonvisual physics data.

</details>

## Next Steps

- [ ] **SIM-028 — Simulation movement never reaches Bevy.**

<details>
<summary>Planned runtime work</summary>

**Locations:** [simulation_sync.rs](src/mujoco_simulation/simulation_sync.rs),
[simulation_step.rs](src/mujoco_simulation/simulation_step.rs),
[body.rs](src/mujoco_simulation/world_items/body.rs).

Recurring pose synchronization is not implemented. Body poses are extracted on
recompilation, while fixed updates advance only MuJoCo.

**Complete when:** required visual poses are synchronized after advancement,
using the coordinate contract established by SIM-001/SIM-002. Define the time
represented by the pose snapshot and refresh derived positions when needed;
`mj_step` integrates after its dynamics evaluation.

</details>

- [ ] **SIM-029 — Simulation time does not match Bevy time.**

<details>
<summary>Planned runtime work</summary>

**Location:** [simulation_step.rs](src/mujoco_simulation/simulation_step.rs).

One MuJoCo step currently runs per Bevy fixed tick. With the defaults used by the
sample, 64 ticks per elapsed second multiplied by a 0.002-second MuJoCo timestep
advances only 0.128 simulation seconds: about 7.8 times slower than real time.

**Complete when:** simulation advancement follows the intended elapsed/virtual
time and playback speed, using each model's timestep. A per-simulation accumulator
and substeps can support models with different timesteps.

[Bevy fixed-time default](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_time/src/fixed.rs),
[MuJoCo timestep default](https://github.com/google-deepmind/mujoco/blob/3.12.0/src/engine/engine_init.c#L51).

</details>

## Non-Issue

- [x] **SIM-030 — Material handles are inserted into the wrong component.**

<details>
<summary>Closed by design</summary>

**Locations:** [assets/mod.rs](src/mujoco_simulation/assets/mod.rs),
[geom.rs](src/mujoco_simulation/world_items/geom.rs).

**Owner's decision:** asset components exist to look up handles. Asset entities
never render anything. Keeping `MujocoAssetHandle<StandardMaterial>` as an asset
lookup component is intentional.

The original finding is closed under that architectural intent. It does not
require adding render components to asset entities. Rendering consumers and
their eventual material-binding work are separate from the asset lookup role.

</details>
