use alloc::sync::Arc;
use spin::Mutex;
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapType {
    Null,
    Mem,
    EP,
    Proc,
    CNode,
}

pub type Rights = u8;
pub const RIGHT_READ:  Rights = 1 << 0;
pub const RIGHT_WRITE: Rights = 1 << 1;
pub const RIGHT_EXEC:  Rights = 1 << 2;
pub const RIGHT_MAP:   Rights = 1 << 3;
pub const RIGHT_SEND:  Rights = 1 << 4;
pub const RIGHT_RECV:  Rights = 1 << 5;
pub const RIGHT_SPAWN: Rights = 1 << 6;
pub const RIGHT_KILL:  Rights = 1 << 7;

pub struct Resource {
    pub ref_count: Mutex<usize>,
    pub res_type: CapType,
    pub derivation_parent: Mutex<Option<usize>>,
    pub derivation_children: Mutex<Vec<usize>>,
    pub data: ResourceData,
}

pub enum ResourceData {
    Null,
    Mem { phys_addr: usize, size: usize },
    EP { ep_id: usize },
    Proc { pid: u32 },
    CNode { slots: Mutex<Vec<Slot>> },
}

#[derive(Clone)]
pub struct Slot {
    pub cap_type: CapType,
    pub rights: Rights,
    pub resource_id: usize,
}

impl Slot {
    pub fn null() -> Self {
        Slot { cap_type: CapType::Null, rights: 0, resource_id: 0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CapRef {
    pub cap_type: CapType,
    pub rights: Rights,
    pub resource_id: usize,
}
