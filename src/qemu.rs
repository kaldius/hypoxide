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
