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

mod qemu;
mod serial;
mod testable;
mod vga_buffer;

// This function is called on panic
#[cfg(not(test))] // only compiled when not `cargo test`
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)] // only compiled when using `cargo test`
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n"); // we want to use serial_println when testing
    serial_println!("Error: {}\n", info);
    qemu::exit_qemu(qemu::QemuExitCode::Failed);
    loop {}
}

// Custom entrypoint
// `pub extern "C"` specifies to use C ABI
// We call it `_start` because it is the default entrypoint name for most systems.
#[unsafe(no_mangle)] // prevents compiler from generating function with a cryptic unique name
pub extern "C" fn _start() -> ! {
    println!("Hello world{}", "!");

    // test_main is only compiled when we call `cargo test`
    #[cfg(test)]
    test_main();

    loop {}
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn testable::Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    qemu::exit_qemu(qemu::QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
