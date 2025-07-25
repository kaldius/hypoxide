# hypoxide

### Prerequisites

#### `bootimage` tool

```sh
cargo install bootimage
```

We already have a bootloader from the `bootloader` crate. However, we need to link our kernel with the bootloader after compilation, but cargo has no support for post-build scripts. `bootimage` solves this problem by first compiling the kernel and bootloader, then linking them together to create a bootable disk image.

### References

Follwing [this guide](https://os.phil-opp.com/)
