#![no_std]
// disable std lib
// normally, we have crt0 (C runtime zero) to invoke the entrypoint of the Rust runtime, but is is
// not available, so we disable the main function
#![no_main]
// `test` depends on std lib, so instead we use this feature requires no external libraries and
// runs all functions annotated with #[test_case]
#![feature(custom_test_frameworks)]
#![test_runner(hypoxide::test_runner)]
// specify our test runner function
// custom_test_frameworks generates a `main` function which calls our test_runner
// this line changes the name of that `main` function to `test_main`
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use hypoxide::println;

// Custom entrypoint
// `pub extern "C"` specifies to use C ABI
// We call it `_start` because it is the default entrypoint name for most systems.
#[unsafe(no_mangle)] // prevents compiler from generating function with a cryptic unique name
pub extern "C" fn _start() -> ! {
    println!("Hello world{}", "!");

    // init IDT
    hypoxide::init();

    fn stack_overflow() {
        stack_overflow();
    }

    stack_overflow();

    // test_main is only compiled when we call `cargo test`
    #[cfg(test)]
    test_main();

    loop {}
}

#[cfg(not(test))] // only compiled when not `cargo test`
#[panic_handler] // This function is called on panic
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    hypoxide::test_panic_handler(info)
}
