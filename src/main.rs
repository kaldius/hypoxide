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

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use hypoxide::{allocator, println};

extern crate alloc;

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

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    let x = Box::new(41);
    println!("x at {:p}", x);

    let mut v = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    println!("vec at {:p}", v.as_slice());

    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!(
        "current reference count {}",
        Rc::strong_count(&cloned_reference)
    );
    core::mem::drop(reference_counted);
    println!(
        "reference count is now {}",
        Rc::strong_count(&cloned_reference)
    );

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
