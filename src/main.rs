#![no_std]
// disable std lib
// normally, we have crt0 (C runtime zero) to invoke the entrypoint of the Rust runtime, but is is
// not available, so we disable the main function
#![no_main]
// `test` depends on std lib, so instead we use this feature requires no external libraries and
// runs all functions annotated with #[test_case]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
// specify our test runner function
// custom_test_frameworks generates a `main` function which calls our test_runner
// this line changes the name of that `main` function to `test_main`
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

mod vga_buffer;

// This function is called on panic
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// Custom entrypoint
// `pub extern "C"` specifies to use C ABI
// We call it `_start` because it is the default entrypoint name for most systems.
#[unsafe(no_mangle)] // prevents compiler from generating function with a cryptic unique name
pub extern "C" fn _start() -> ! {
    println!("Hello world{}", "!");

    // test_main is only called when we call `cargo test`
    #[cfg(test)]
    test_main();

    loop {}
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)] // port size is 4 bytes (see cargo.toml iosize)
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

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
