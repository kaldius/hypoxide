#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_utils::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

pub mod gdt;
pub mod interrupts;
pub mod qemu;
pub mod serial;
pub mod test_utils;
pub mod vga_buffer;

#[cfg(test)]
use core::panic::PanicInfo;

pub fn init() {
    // needs to happen before init_idt because double fault handler depends on the IST entry set up here
    gdt::init();
    interrupts::init_idt();
    interrupts::init_pics();
    x86_64::instructions::interrupts::enable();
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_utils::test_panic_handler(info)
}
