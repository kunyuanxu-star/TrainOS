pub fn sys_spawn(_elf_ptr: usize, _elf_len: usize) -> Result<usize, &'static str> {
    // In a real implementation, copy ELF from user space
    // For now, this is a placeholder
    Err("spawn not implemented via syscall")
}

pub fn sys_exit(code: i32) -> Result<usize, &'static str> {
    let current = crate::sched::current_thread().ok_or("no thread")?;
    unsafe { (*current).state = crate::proc::thread::ThreadState::Dead; }
    crate::sched::schedule();
    // Never returns
    loop { unsafe { core::arch::asm!("wfi"); } }
}
