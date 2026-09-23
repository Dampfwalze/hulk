use bevy::{
    color::ColorToComponents,
    math::{DQuat, DVec3, Vec3},
    prelude::{LinearRgba, Quat},
};

/// Converts MuJoCo values to Bevy without changing the coordinate basis.
///
/// Implement [`BevyFromMujoco`] to provide this trait automatically.
pub(super) trait MujocoToBevy<Output> {
    fn to_bevy(&self) -> Output;
}

/// Converts Bevy values to MuJoCo without changing the coordinate basis.
///
/// Implement [`MujocoFromBevy`] to provide this trait automatically.
pub(super) trait BevyToMujoco<Output> {
    fn to_mujoco(&self) -> Output;
}

/// Constructs a Bevy value from MuJoCo data without changing the coordinate basis.
///
/// Like [`From`], implementing this also provides the corresponding source-side
/// conversion. Inputs are borrowed to match [`MujocoToBevy::to_bevy`].
pub(super) trait BevyFromMujoco<Input: ?Sized>: Sized {
    fn from_mujoco(value: &Input) -> Self;
}

/// Constructs MuJoCo data from a Bevy value without changing the coordinate basis.
///
/// Like [`From`], implementing this also provides the corresponding source-side
/// conversion. Inputs are borrowed to match [`BevyToMujoco::to_mujoco`].
pub(super) trait MujocoFromBevy<Input: ?Sized>: Sized {
    fn from_bevy(value: &Input) -> Self;
}

impl<Input: ?Sized, Output: BevyFromMujoco<Input>> MujocoToBevy<Output> for Input {
    fn to_bevy(&self) -> Output {
        Output::from_mujoco(self)
    }
}

impl<Input: ?Sized, Output: MujocoFromBevy<Input>> BevyToMujoco<Output> for Input {
    fn to_mujoco(&self) -> Output {
        Output::from_bevy(self)
    }
}

impl BevyFromMujoco<[f64; 3]> for Vec3 {
    fn from_mujoco(value: &[f64; 3]) -> Self {
        DVec3::from_mujoco(value).as_vec3()
    }
}

impl MujocoFromBevy<Vec3> for [f64; 3] {
    fn from_bevy(value: &Vec3) -> Self {
        Self::from_bevy(&value.as_dvec3())
    }
}

impl BevyFromMujoco<[f64; 3]> for DVec3 {
    fn from_mujoco(value: &[f64; 3]) -> Self {
        Self::from_array(*value)
    }
}

impl MujocoFromBevy<DVec3> for [f64; 3] {
    fn from_bevy(value: &DVec3) -> Self {
        value.to_array()
    }
}

impl BevyFromMujoco<[f64; 4]> for Quat {
    fn from_mujoco(value: &[f64; 4]) -> Self {
        DQuat::from_mujoco(value).as_quat()
    }
}

impl MujocoFromBevy<Quat> for [f64; 4] {
    fn from_bevy(value: &Quat) -> Self {
        Self::from_bevy(&value.as_dquat())
    }
}

impl BevyFromMujoco<[f64; 4]> for DQuat {
    fn from_mujoco(value: &[f64; 4]) -> Self {
        // MuJoCo stores scalar-first quaternions; Bevy stores scalar-last.
        let [w, x, y, z] = *value;
        Self::from_xyzw(x, y, z, w)
    }
}

impl MujocoFromBevy<DQuat> for [f64; 4] {
    fn from_bevy(value: &DQuat) -> Self {
        [value.w, value.x, value.y, value.z]
    }
}

impl BevyFromMujoco<[f32; 4]> for LinearRgba {
    fn from_mujoco(value: &[f32; 4]) -> Self {
        Self::from_f32_array(*value)
    }
}

impl MujocoFromBevy<LinearRgba> for [f32; 4] {
    fn from_bevy(value: &LinearRgba) -> Self {
        value.to_f32_array()
    }
}
