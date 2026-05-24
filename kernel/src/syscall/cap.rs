use crate::cap::ops;
use crate::cap::types;

/// Get the CNode resource ID of the calling process.
fn caller_cnode() -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter().find(|p| p.pid == pid).ok_or("no process")?;
    Ok(proc.cnode_id)
}

/// sys_mint(src_slot_idx, desired_rights) -> new_slot_idx
/// Derive a new capability from an existing one in the caller's CNode.
pub fn sys_mint(src_idx: usize, desired_rights: u8) -> Result<usize, &'static str> {
    let cnode = caller_cnode()?;

    // V21: Deep validation — child rights must be subset of parent rights
    let parent_rights = {
        let res = ops::get_resource(cnode).ok_or("cnode gone")?;
        if let types::ResourceData::CNode { ref slots } = &res.data {
            let s = slots.lock();
            s.get(src_idx).map(|slot| slot.rights).unwrap_or(0)
        } else {
            0
        }
    };
    if (desired_rights & !parent_rights) != 0 {
        let pid = crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner })
            .unwrap_or(0);
        crate::security::cap_audit_log(pid, 4, src_idx);
        return Err("rights escalation denied");
    }

    let slot = ops::mint(cnode, src_idx, desired_rights).ok_or("mint failed")?;

    // Append the minted slot to the caller's CNode
    let res = ops::get_resource(cnode).ok_or("cnode gone")?;
    if let types::ResourceData::CNode { ref slots } = &res.data {
        let mut s = slots.lock();
        s.push(slot);
        let idx = s.len() - 1;
        return Ok(idx);
    }
    Err("mint: cnode resource invalid")
}

/// sys_copy(src_idx, dst_pid, dst_idx) -> 0
/// Copy a capability from the caller's CNode to another process's CNode.
pub fn sys_copy(src_idx: usize, dst_pid: u32, dst_idx: usize) -> Result<usize, &'static str> {
    let src_cnode = caller_cnode()?;
    let procs = crate::proc::PROCESSES.lock();
    let dst_proc = procs
        .iter()
        .find(|p| p.pid == dst_pid)
        .ok_or("dst process not found")?;
    let dst_cnode = dst_proc.cnode_id;
    drop(procs);
    ops::copy_cap(src_cnode, src_idx, dst_cnode, dst_idx)?;
    Ok(0)
}

/// sys_move(src_idx, dst_pid, dst_idx) -> 0
/// Move a capability from the caller's CNode to another process's CNode.
pub fn sys_move(src_idx: usize, dst_pid: u32, dst_idx: usize) -> Result<usize, &'static str> {
    let src_cnode = caller_cnode()?;
    let procs = crate::proc::PROCESSES.lock();
    let dst_proc = procs
        .iter()
        .find(|p| p.pid == dst_pid)
        .ok_or("dst process not found")?;
    let dst_cnode = dst_proc.cnode_id;
    drop(procs);
    ops::move_cap(src_cnode, src_idx, dst_cnode, dst_idx)?;
    Ok(0)
}

/// sys_delete(slot_idx) -> 0
/// Delete a capability from the caller's CNode.
pub fn sys_delete(slot_idx: usize) -> Result<usize, &'static str> {
    let cnode = caller_cnode()?;
    ops::delete_cap(cnode, slot_idx)?;
    Ok(0)
}

/// Return capability statistics for the calling process.
/// Format: [total_slots:16][used_slots:16][ep_caps:16][mem_caps:16] packed into usize
pub fn sys_cap_stats() -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter().find(|p| p.pid == pid).ok_or("no process")?;
    let cnode_id = proc.cnode_id;
    drop(procs);

    let res = ops::get_resource(cnode_id).ok_or("no cnode")?;
    if let types::ResourceData::CNode { ref slots } = &res.data {
        let slots = slots.lock();
        let total = slots.len();
        let mut used: usize = 0;
        let mut ep_count: usize = 0;
        let mut mem_count: usize = 0;

        for slot in slots.iter() {
            match slot.cap_type {
                types::CapType::Null => {}
                types::CapType::EP => {
                    used += 1;
                    ep_count += 1;
                }
                types::CapType::Mem => {
                    used += 1;
                    mem_count += 1;
                }
                _ => {
                    used += 1;
                }
            }
        }

        // Pack into a usize: [total:16][used:16][ep:16][mem:16]
        let result = (total & 0xFFFF)
            | ((used & 0xFFFF) << 16)
            | ((ep_count & 0xFFFF) << 32)
            | ((mem_count & 0xFFFF) << 48);

        Ok(result)
    } else {
        Err("not a cnode")
    }
}
