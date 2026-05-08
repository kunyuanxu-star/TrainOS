use super::types::*;
use super::{NEXT_RESOURCE_ID, RESOURCES};
use alloc::vec::Vec;
use spin::Mutex;

pub fn alloc_resource(res_type: CapType, data: ResourceData) -> usize {
    let mut next = NEXT_RESOURCE_ID.lock();
    let id = *next;
    *next += 1;
    let resource = Resource {
        ref_count: Mutex::new(1),
        res_type,
        derivation_parent: Mutex::new(None),
        derivation_children: Mutex::new(Vec::new()),
        data,
    };
    let mut resources = RESOURCES.lock();
    while resources.len() <= id {
        resources.push(None);
    }
    resources[id] = Some(resource);
    id
}

pub fn get_resource(id: usize) -> Option<&'static Resource> {
    let resources = RESOURCES.lock();
    if id < resources.len() && resources[id].is_some() {
        // SAFETY: Resources are never deallocated while referenced.
        // We leak a reference to return a static lifetime.
        let ptr = resources[id].as_ref().unwrap() as *const Resource;
        Some(unsafe { &*ptr })
    } else {
        None
    }
}

pub fn inc_ref(id: usize) {
    let resources = RESOURCES.lock();
    if let Some(ref res) = resources[id] {
        *res.ref_count.lock() += 1;
    }
}

pub fn dec_ref(id: usize) {
    let mut resources = RESOURCES.lock();
    if id < resources.len() {
        if let Some(ref res) = resources[id] {
            let mut refcnt = res.ref_count.lock();
            *refcnt -= 1;
            if *refcnt == 0 {
                drop(refcnt);
                resources[id] = None;
            }
        }
    }
}

pub fn mint(cnode_id: usize, src_slot_idx: usize, desired_rights: Rights) -> Option<Slot> {
    let resources = RESOURCES.lock();
    // Extract the slot data within a narrow scope so borrows on `resources` are dropped
    // before we access `resources` again for derivation tracking.
    let (src_cap_type, _src_rights, src_resource_id) = {
        let cnode = resources.get(cnode_id)?.as_ref()?;
        if let ResourceData::CNode { ref slots } = &cnode.data {
            let slots_guard = slots.lock();
            let src = slots_guard.get(src_slot_idx)?;
            if src.cap_type == CapType::Null {
                return None;
            }
            if (desired_rights & src.rights) != desired_rights {
                return None;
            }
            (src.cap_type, src.rights, src.resource_id)
        } else {
            return None;
        }
    };

    let child_id = alloc_resource(
        src_cap_type,
        duplicate_resource_data_inner(src_resource_id)?,
    );

    // Record derivation
    if let Some(ref parent) = resources[src_resource_id] {
        parent.derivation_children.lock().push(child_id);
    }
    if let Some(ref child) = resources[child_id] {
        *child.derivation_parent.lock() = Some(src_resource_id);
    }

    Some(Slot {
        cap_type: src_cap_type,
        rights: desired_rights,
        resource_id: child_id,
    })
}

fn duplicate_resource_data_inner(resource_id: usize) -> Option<ResourceData> {
    let resources = RESOURCES.lock();
    let res = resources.get(resource_id)?.as_ref()?;
    inc_ref(resource_id);
    match &res.data {
        ResourceData::Null => Some(ResourceData::Null),
        ResourceData::Mem { phys_addr, size } => Some(ResourceData::Mem {
            phys_addr: *phys_addr,
            size: *size,
        }),
        ResourceData::EP { ep_id } => Some(ResourceData::EP { ep_id: *ep_id }),
        ResourceData::Proc { pid } => Some(ResourceData::Proc { pid: *pid }),
        ResourceData::CNode { slots } => {
            let new_slots = slots.lock().clone();
            Some(ResourceData::CNode {
                slots: Mutex::new(new_slots),
            })
        }
    }
}

pub fn copy_cap(
    src_cnode: usize,
    src_idx: usize,
    dst_cnode: usize,
    dst_idx: usize,
) -> Result<(), &'static str> {
    let resources = RESOURCES.lock();

    let src_slot = {
        let src_node = resources
            .get(src_cnode)
            .ok_or("src cnode not found")?
            .as_ref()
            .ok_or("src cnode freed")?;
        if let ResourceData::CNode { ref slots } = &src_node.data {
            slots.lock().get(src_idx).cloned().unwrap_or(Slot::null())
        } else {
            return Err("src not a cnode");
        }
    };

    if src_slot.cap_type == CapType::Null {
        return Err("src slot null");
    }

    inc_ref(src_slot.resource_id);

    let dst_node = resources
        .get(dst_cnode)
        .ok_or("dst cnode not found")?
        .as_ref()
        .ok_or("dst cnode freed")?;
    if let ResourceData::CNode { ref slots } = &dst_node.data {
        let mut slots = slots.lock();
        while slots.len() <= dst_idx {
            slots.push(Slot::null());
        }
        slots[dst_idx] = src_slot;
    }

    Ok(())
}

pub fn move_cap(
    src_cnode: usize,
    src_idx: usize,
    dst_cnode: usize,
    dst_idx: usize,
) -> Result<(), &'static str> {
    copy_cap(src_cnode, src_idx, dst_cnode, dst_idx)?;

    let resources = RESOURCES.lock();
    let src_node = resources.get(src_cnode).unwrap().as_ref().unwrap();
    if let ResourceData::CNode { ref slots } = &src_node.data {
        slots.lock()[src_idx] = Slot::null();
    }

    Ok(())
}

pub fn revoke(resource_id: usize) {
    let resources = RESOURCES.lock();
    if let Some(ref res) = resources[resource_id] {
        let children: Vec<usize> = res.derivation_children.lock().clone();
        drop(resources);
        for child_id in children {
            revoke(child_id);
        }
        dec_ref(resource_id);
    }
}

pub fn delete_cap(cnode_id: usize, slot_idx: usize) -> Result<(), &'static str> {
    let resources = RESOURCES.lock();
    let cnode = resources
        .get(cnode_id)
        .ok_or("cnode not found")?
        .as_ref()
        .ok_or("cnode freed")?;
    if let ResourceData::CNode { ref slots } = &cnode.data {
        let mut slots = slots.lock();
        if slot_idx < slots.len() {
            let rid = slots[slot_idx].resource_id;
            slots[slot_idx] = Slot::null();
            drop(slots);
            drop(resources);
            dec_ref(rid);
            Ok(())
        } else {
            Err("slot out of range")
        }
    } else {
        Err("not a cnode")
    }
}
