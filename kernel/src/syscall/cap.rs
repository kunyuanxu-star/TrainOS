use crate::cap::types::Rights;

pub fn sys_mint(cnode: usize, src_idx: usize, rights: u8) -> Result<usize, &'static str> {
    crate::cap::ops::mint(cnode, src_idx, rights).map(|_| 0).ok_or("mint failed")
}

pub fn sys_copy(src_cn: usize, src_idx: usize, dst_cn: usize, dst_idx: usize) -> Result<usize, &'static str> {
    crate::cap::ops::copy_cap(src_cn, src_idx, dst_cn, dst_idx).map(|_| 0)
}

pub fn sys_move(src_cn: usize, src_idx: usize, dst_cn: usize, dst_idx: usize) -> Result<usize, &'static str> {
    crate::cap::ops::move_cap(src_cn, src_idx, dst_cn, dst_idx).map(|_| 0)
}

pub fn sys_delete(cnode: usize, slot_idx: usize) -> Result<usize, &'static str> {
    crate::cap::ops::delete_cap(cnode, slot_idx).map(|_| 0)
}
