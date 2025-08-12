#![no_std]
// disable std lib
// normally, we have crt0 (C runtime zero) to invoke the entrypoint of the Rust runtime, but is is
// not available, so we disable the main function
#![no_main]
// `test` depends on std lib, so instead we use this feature requires no external libraries and
// runs all functions annotated with #[test_case]
#![feature(custom_test_frameworks)]
#![test_runner(hypoxide::test_utils::test_runner)]
// specify our test runner function
// custom_test_frameworks generates a `main` function which calls our test_runner
// this line changes the name of that `main` function to `test_main`
#![reexport_test_harness_main = "test_main"]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use hypoxide::println;

// Creates the `_start` entrypoint function for us
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use hypoxide::memory::active_level_4_table;
    use x86_64::VirtAddr;

    println!("Hello world{}", "!");
    hypoxide::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(phys_mem_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        if !entry.is_unused() {
            println!("L4 Entry {}: {:?}", i, entry);
            // TODO: continue at "Translating Addresses"
        }
    }

    // test_main is only compiled when we call `cargo test`
    #[cfg(test)]
    test_main();

    println!("It did not crash");
    hypoxide::hlt_loop();
}

#[cfg(not(test))] // only compiled when not `cargo test`
#[panic_handler] // This function is called on panic
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hypoxide::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    hypoxide::test_utils::test_panic_handler(info)
}
