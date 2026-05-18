use super::thread::Thread;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessState {
    Ready,
    Running,
    Waiting,
    Dead,
}

pub struct Process {
    pub pid: u32,
    pub state: ProcessState,
    pub base_priority: u8,
    pub page_table_root: usize,
    pub thread: Option<Thread>,
    pub parent: Option<u32>,
    pub cnode_id: usize,
    pub uid: u32,  // user ID (0 = root)
    pub gid: u32,  // group ID
    pub umask: u16,
}

impl Process {
    pub fn new(pid: u32, priority: u8, page_table_root: usize, cnode_id: usize, uid: u32, gid: u32) -> Self {
        Process {
            pid,
            state: ProcessState::Ready,
            base_priority: priority,
            page_table_root,
            thread: None,
            parent: None,
            cnode_id,
            uid,
            gid,
            umask: 0o022,
        }
    }
}
