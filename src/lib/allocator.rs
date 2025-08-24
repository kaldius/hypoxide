use bump::BumpAllocator;
use spin::MutexGuard;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
    },
};

pub mod bump;
pub mod linked_list;

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;

        // Create start end end pages from addresses
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);

        // Create page range
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        // Allocate a frame to map to
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // Map the every page to the new frame
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

/// A generic wrapper around spin::Mutex to get around the orphan rules
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> MutexGuard<A> {
        self.inner.lock()
    }
}

/// Align the given address upwards to alignment
///
/// Requires that `alignment` is a power of two, which is guaranteed by
/// core::alloc::Layout::align()
fn align_up(addr: usize, alignment: usize) -> usize {
    // e.g.
    // alignment = 8    = 0...01000
    // alignment - 1    = 0...00111
    // !(alignment - 1) = 1...11000
    // addr & !(alignment - 1) will remove the last 3 bits, effectively aligning downwards
    // since we want to align upwards, we add (alignment - 1) first
    let ones_below = alignment - 1;
    let align_down_mask = !(ones_below);
    (addr + ones_below) & align_down_mask
}
