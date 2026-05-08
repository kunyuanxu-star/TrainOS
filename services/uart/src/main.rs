#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

// UART16550 registers (offsets from base)
const UART_THR: usize = 0; // Transmit Holding Register (write)

// Machina's UART is at physical address 0x10000000
const UART_BASE: usize = 0x1000_0000;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("UART: mapping MMIO...\r\n");

    let va = tros::map_mmio(UART_BASE, 0x1000);
    if va == 0 {
        tros::print("UART: map_mmio failed!\r\n");
        loop {
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }

    tros::print("UART: mapped at va=0x");
    // Print VA in hex (simple)
    for i in (0..8).rev() {
        let nibble = (va >> (i * 4)) & 0xF;
        let c = if nibble < 10 {
            b'0' + nibble as u8
        } else {
            b'a' + (nibble - 10) as u8
        };
        tros::putchar(c);
    }
    tros::print("\r\n");

    // Write a test character directly to UART THR
    let thr_addr = va + UART_THR;
    unsafe {
        (thr_addr as *mut u8).write_volatile(b'!');
    }
    tros::print("\r\nUART: wrote char via MMIO!\r\n");

    // Write "UART_OK\r\n" byte by byte via MMIO
    let msg = b"\r\nUART_OK\r\n";
    for &byte in msg.iter() {
        unsafe {
            (thr_addr as *mut u8).write_volatile(byte);
        }
        // Small delay for UART
        for _ in 0..100 {
            unsafe {
                core::arch::asm!("nop");
            }
        }
    }

    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
