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
    use hypoxide::memory;
    use x86_64::VirtAddr;
    use x86_64::structures::paging::Translate;

    println!("Hello world{}", "!");
    hypoxide::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = mapper.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
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
