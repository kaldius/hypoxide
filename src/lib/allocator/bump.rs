use core::{
    alloc::{GlobalAlloc, Layout},
    cmp, ptr,
};

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for super::Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // GlobalAlloc::alloc must borrow self immutably because the #[global_allocator] is `static`,
        // which is immutable by definition, so we use spin::Mutex for interior mutability
        let mut locked_self = self.lock();

        let alloc_start = super::align_up(locked_self.next, layout.align());

        // Guard against overflow
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(a) => a,
            None => return ptr::null_mut(),
        };

        if alloc_end > locked_self.heap_end {
            ptr::null_mut()
        } else {
            locked_self.allocations += 1;
            locked_self.next = alloc_end;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut locked_self = self.lock();

        locked_self.allocations = cmp::max(0, locked_self.allocations - 1);
        if locked_self.allocations == 0 {
            locked_self.next = locked_self.heap_start;
        }
    }
}
