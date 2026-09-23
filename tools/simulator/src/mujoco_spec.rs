use std::sync::RwLock;

use bevy::platform::cell::SyncCell;
use mujoco_rs::wrappers::{MjSpec, mj_editing::SendableSpec};

pub struct SendSyncSpec(SyncCell<SendableSpec>);

#[test]
fn ensure_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<SendSyncSpec>();
}
