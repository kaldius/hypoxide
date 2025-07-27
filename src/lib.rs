#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod serial;
pub mod vga_buffer;

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        crate::serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        crate::serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n"); // we want to use serial_println when testing
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)] // port size is 4 bytes (see cargo.toml iosize)
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

#[allow(dead_code)]
pub fn exit_qemu(exit_code: QemuExitCode) {
    // Helps with reading/writing from/to port-mapped I/O
    use x86_64::instructions::port::Port;

    unsafe {
        // qemu isa-debug-exit lives at this port address (see cargo.toml iobase)
        let mut port = Port::new(0xf4);
        // qemu_exit_code will be (exit_code << 1) | 1
        port.write(exit_code as u32);
    }
}
