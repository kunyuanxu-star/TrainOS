// Device Driver Framework — standardized driver ABI for user-space drivers
//
// Drivers run as user-space services. The kernel provides:
//   1. Driver registration (name, type, probe function endpoint)
//   2. Device discovery notification
//   3. Standardized MMIO access (map, read32, write32)
//   4. Interrupt delivery (future)
//
// Driver types:
//   DRV_BLOCK   = 1  — block device driver
//   DRV_NET     = 2  — network device driver
//   DRV_CHAR    = 3  — character device driver
//   DRV_PCI     = 4  — PCI device driver
//   DRV_DISPLAY = 5  — display driver

pub const DRV_BLOCK: u32 = 1;
pub const DRV_NET: u32 = 2;
pub const DRV_CHAR: u32 = 3;
pub const DRV_PCI: u32 = 4;

const MAX_DRIVERS: usize = 16;

struct Driver {
    name: [u8; 32],
    name_len: usize,
    drv_type: u32,
    pid: u32,          // process that registered this driver
    ep: usize,         // endpoint for probing
    registered: bool,
}

static mut DRIVERS: [Driver; MAX_DRIVERS] = [
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
];

/// Register a driver. Returns driver ID.
pub fn register(name: &[u8], drv_type: u32, pid: u32, ep: usize) -> Option<usize> {
    unsafe {
        for i in 0..MAX_DRIVERS {
            if !DRIVERS[i].registered {
                DRIVERS[i].registered = true;
                DRIVERS[i].drv_type = drv_type;
                DRIVERS[i].pid = pid;
                DRIVERS[i].ep = ep;
                let nlen = name.len().min(31);
                DRIVERS[i].name_len = nlen;
                for j in 0..nlen { DRIVERS[i].name[j] = name[j]; }
                return Some(i);
            }
        }
    }
    None
}

/// Unregister a driver.
pub fn unregister(drv_id: usize) -> bool {
    unsafe {
        if drv_id < MAX_DRIVERS && DRIVERS[drv_id].registered {
            DRIVERS[drv_id].registered = false;
            return true;
        }
    }
    false
}

/// List drivers. Fills buf with driver info.
/// Format per driver: [type:4][pid:4][name_len:1][name:name_len]
pub fn list(buf: &mut [u8]) -> usize {
    let mut pos: usize = 0;
    unsafe {
        for i in 0..MAX_DRIVERS {
            if !DRIVERS[i].registered { continue; }
            let need = 9 + DRIVERS[i].name_len;
            if pos + need > buf.len() { break; }

            // type (4 bytes)
            let t = DRIVERS[i].drv_type;
            buf[pos] = t as u8; buf[pos+1] = (t>>8) as u8; buf[pos+2] = (t>>16) as u8; buf[pos+3] = (t>>24) as u8;

            // pid (4 bytes)
            let p = DRIVERS[i].pid;
            buf[pos+4] = p as u8; buf[pos+5] = (p>>8) as u8; buf[pos+6] = (p>>16) as u8; buf[pos+7] = (p>>24) as u8;

            // name_len
            buf[pos+8] = DRIVERS[i].name_len as u8;

            // name
            for j in 0..DRIVERS[i].name_len {
                buf[pos+9+j] = DRIVERS[i].name[j];
            }

            pos += need;
        }
    }
    pos
}

/// Probe a device with a registered driver.
pub fn probe_pci(vendor: u16, device: u16) -> Option<(usize, u32)> {
    // For now, return the first registered PCI driver
    unsafe {
        for i in 0..MAX_DRIVERS {
            if DRIVERS[i].registered && DRIVERS[i].drv_type == DRV_PCI {
                return Some((DRIVERS[i].ep, DRIVERS[i].pid));
            }
        }
    }
    None
}
