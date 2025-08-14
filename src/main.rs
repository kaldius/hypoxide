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
use x86_64::structures::paging::Page;

// Creates the `_start` entrypoint function for us
entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use hypoxide::memory;
    use x86_64::VirtAddr;

    println!("Hello world{}", "!");
    hypoxide::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    let page = Page::containing_address(VirtAddr::new(0xdeadbeef));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };

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
