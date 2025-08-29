use super::Locked;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::{self, NonNull},
};

struct ListNode {
    next: Option<&'static mut ListNode>,
}

/// Block sizes to use.
///
/// Powers of 2 because blocks will also be aligned according to their size (to simplify
/// implementation).
/// Min of 8 bytes because each node must hold a 64-bit pointer to the next.
/// For allocations more than 2048 bytes, we will use the fallback allocator.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialise the allocator with the given heap bounds.
    ///
    /// Unsafe because the caller must guarantee the given heap bounds are valid and that the heap
    /// is unused. This method can only be called once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size) };
    }

    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}

/// Choose an appropriate block size for the given layout.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(idx) => match allocator.list_heads[idx].take() {
                Some(node) => {
                    allocator.list_heads[idx] = node.next.take();
                    node as *mut ListNode as *mut u8
                }
                None => {
                    let size = BLOCK_SIZES[idx];
                    let align = size;
                    let new_layout = Layout::from_size_align(size, align).unwrap();
                    allocator.fallback_alloc(new_layout)
                }
            },
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(idx) => {
                let new_node = ListNode {
                    next: allocator.list_heads[idx].take(),
                };
                let new_node_ptr = ptr as *mut ListNode;
                unsafe {
                    new_node_ptr.write(new_node);
                    allocator.list_heads[idx] = Some(&mut *new_node_ptr)
                };
            }
            None => {
                let ptr = NonNull::new(ptr).unwrap();
                unsafe {
                    allocator.fallback_allocator.deallocate(ptr, layout);
                }
            }
        }
    }
}
