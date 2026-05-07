use crate::cap::types::CapRef;

pub const MAX_PAYLOAD: usize = 64;
pub const MAX_CAP_TRANSFER: usize = 4;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct CapTransfer {
    pub src_slot: u32,
    pub dst_slot: u32,
    pub mode: TransferMode,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransferMode { Copy, Move }

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Message {
    pub sender_pid: u32,
    pub opcode: u16,
    pub payload: [u8; MAX_PAYLOAD],
    pub payload_len: usize,
    pub cap_transfers: [Option<CapTransfer>; MAX_CAP_TRANSFER],
}

impl Message {
    pub fn new(sender_pid: u32, opcode: u16) -> Self {
        Message {
            sender_pid, opcode,
            payload: [0; MAX_PAYLOAD],
            payload_len: 0,
            cap_transfers: [None; MAX_CAP_TRANSFER],
        }
    }
}
