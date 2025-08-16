# hypoxide

## About This Project

This is a work-in-progress operating system written in Rust, built by following [Philipp Oppermann’s excellent blog series](https://os.phil-opp.com/).  
Most of the code currently comes directly from the guide, but I am using this project as a way to learn low-level systems programming, memory management, and Rust’s safety guarantees.

My goal is to extend this OS beyond the tutorial by experimenting with:

- New strategies for:
    - memory management,
    - scheduling
- Custom drivers
- Exploring concurrency in a `no_std` environment

## Prerequisites

### `bootimage` tool

```sh
cargo install bootimage
```

We already have a bootloader from the `bootloader` crate. However, we need to link our kernel with the bootloader after compilation, but cargo has no support for post-build scripts. `bootimage` solves this problem by first compiling the kernel and bootloader, then linking them together to create a bootable disk image.

## Resources

- [ostep book](https://pages.cs.wisc.edu/~remzi/OSTEP/)
- [xv6](https://github.com/mit-pdos/xv6-public)
