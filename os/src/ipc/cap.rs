//! Capability-based security module

use crate::ipc::PortId;

/// Capability rights
#[derive(Debug, Clone, Copy)]
pub struct CapRights(pub u32);

impl CapRights {
    pub const NONE: CapRights = CapRights(0);
    pub const READ: CapRights = CapRights(1 << 0);
    pub const WRITE: CapRights = CapRights(1 << 1);
    pub const EXECUTE: CapRights = CapRights(1 << 2);
    pub const GRANT: CapRights = CapRights(1 << 3);

    pub fn contains(&self, right: CapRights) -> bool {
        (self.0 & right.0) != 0
    }
}

/// Capability
#[derive(Debug, Clone, Copy)]
pub struct Cap {
    pub id: u64,
    pub port: PortId,
    pub rights: CapRights,
}

impl Cap {
    pub fn new(port: PortId, rights: CapRights) -> Self {
        Self {
            id: rand_u64(),
            port,
            rights,
        }
    }

    pub fn can_send(&self) -> bool {
        self.rights.contains(CapRights::WRITE)
    }

    pub fn can_recv(&self) -> bool {
        self.rights.contains(CapRights::READ)
    }

    pub fn can_grant(&self) -> bool {
        self.rights.contains(CapRights::GRANT)
    }
}

/// Generate a random capability ID using cycle counter
fn rand_u64() -> u64 {
    let mut val: u64;
    unsafe {
        core::arch::asm!("rdtime {0}", out(reg) val);
    }
    val
}