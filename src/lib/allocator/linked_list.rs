use core::{
    alloc::{GlobalAlloc, Layout},
    mem, ptr,
};

pub struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode {
            size: size,
            next: None,
        }
    }

    pub fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    pub fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }

    /// Given an allocation layout, determines the size to allocate such that:
    /// 1. the request is met, and
    /// 2. the allocated region will fit a ListNode when eventually freed.
    fn alloc_size(layout: Layout) -> usize {
        layout.size().max(mem::size_of::<Self>())
    }

    /// Given an allocation layout, determines the alignment such that:
    /// 1. the request is met, and
    /// 2. the allocated region will be properly aligned when a replaced with a ListNode after it
    ///    it is freed.
    fn alloc_align(layout: Layout) -> usize {
        layout
            .align_to(mem::align_of::<Self>())
            .unwrap()
            .pad_to_align()
            .align()
    }

    /// Attempts to allocate a memory region in the ListNode.
    /// Returns a UsableRegion with the allocated region.
    /// If there is extra space in the ListNode, the returned UsableRegion will also contain an
    /// excess_region.
    ///
    /// Returns Err if:
    /// 1. the ListNode is too small for the requested size, or
    /// 2. the leftover size is too small to fit a new ListNode.
    fn try_allocate(&self, size: usize, align: usize) -> Result<UsableRegion, ()> {
        let alloc_start = self.start_addr();

        // This assertion is already done by LinkedListAllocator.add_free_region()
        assert_eq!(super::align_up(alloc_start, align), alloc_start);

        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > self.end_addr() {
            return Err(());
        }

        let alloc_region = MemRegion {
            start: alloc_start,
            size,
        };

        let remaining_size = self.end_addr() - alloc_end;
        if remaining_size > 0 {
            if remaining_size < mem::size_of::<Self>() {
                // If there is remaining space, ensure it is large enough to fit another ListNode
                return Err(());
            }
            let excess_region = MemRegion {
                start: alloc_end,
                size: remaining_size,
            };
            return Ok(UsableRegion {
                alloc_region,
                excess_region: Some(excess_region),
            });
        }

        Ok(UsableRegion {
            alloc_region,
            excess_region: None,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct UsableRegion {
    alloc_region: MemRegion,
    excess_region: Option<MemRegion>,
}

#[derive(Debug, PartialEq, Eq)]
struct MemRegion {
    start: usize,
    size: usize,
}

pub struct LinkedListAllocator {
    // Dummy ListNode whose `next` points to the first node
    head: ListNode,
}

/// A heap allocator that writes ListNodes into the heap, forming a Linked List.
///
/// TODO: Does not merge freed blocks for now.
impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: ListNode::new(0),
        }
    }

    /// Initialise the allocator with the given heap bounds.
    ///
    /// Unsafe because the caller must guarantee the given heap bounds are valid and that the heap
    /// is unused. This method can only be called once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// Adds a ListNode to the front of the LL, representing addr and size.
    ///
    /// Unsafe because the caller must make sure addr is a valid address on the heap which can be
    /// written over.
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(
            super::align_up(addr, mem::align_of::<ListNode>()),
            addr,
            "addr must be aligned to ListNode"
        );
        assert!(
            size >= mem::size_of::<ListNode>(),
            "size must be big enough to fit a ListNode"
        );

        let mut node = ListNode::new(size);
        // put new node in front of the LL
        node.next = self.head.next.take();

        // create a ListNode pointer from the given addr (which is in the heap)
        let node_ptr = addr as *mut ListNode;
        unsafe {
            // write node (currently on stack) into heap ptr
            node_ptr.write(node);
            // set head.next to the heap copy of node
            self.head.next = Some(&mut *node_ptr);
        }
    }

    /// Looks for a free region with the given size and alignment and removes it from the list.
    ///
    /// Returns a UsableRegion which may contain an excess_region.
    fn extract_first_suitable_region(&mut self, size: usize, align: usize) -> Option<UsableRegion> {
        let mut curr = &mut self.head;
        while let Some(ref mut node) = curr.next {
            if let Ok(usable_region) = node.try_allocate(size, align) {
                let next = node.next.take();
                curr.next = next;
                return Some(usable_region);
            } else {
                curr = curr.next.as_mut().unwrap();
            }
        }
        None
    }
}

unsafe impl GlobalAlloc for super::Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = ListNode::alloc_size(layout);
        let align = ListNode::alloc_align(layout);
        let mut allocator = self.lock();
        let Some(UsableRegion {
            alloc_region,
            excess_region,
        }) = allocator.extract_first_suitable_region(size, align)
        else {
            return ptr::null_mut();
        };

        // If there is an excess_region, add it back to the LL
        if let Some(MemRegion { start, size }) = excess_region {
            unsafe {
                allocator.add_free_region(start, size);
            }
        }

        alloc_region.start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // TODO: check that ptr is within the original inited region
        unsafe {
            self.lock()
                .add_free_region(ptr as usize, ListNode::alloc_size(layout));
        }
    }
}

#[cfg(test)]
mod list_node {
    use super::*;

    #[test_case]
    fn assert_size_and_align() {
        assert_eq!(
            mem::size_of::<ListNode>(),
            16,
            "ListNode should always be 16 bytes large"
        );
        assert_eq!(
            mem::align_of::<ListNode>(),
            8,
            "ListNode should always be aligned to 8 bytes"
        )
    }

    mod try_allocate {
        use super::*;

        #[test_case]
        fn size_too_large() {
            let node = ListNode {
                size: 16,
                next: None,
            };
            assert_eq!(node.try_allocate(32, 16), Err(()));
        }

        #[test_case]
        fn remaining_size_insufficient() {
            let node = ListNode {
                size: 47,
                next: None,
            };
            // 47 - 32 = 15, but we need 16 bytes to fit a ListNode in the excess
            assert_eq!(node.try_allocate(32, 16), Err(()));
        }

        #[test_case]
        fn exact_fit() {
            let node = ListNode {
                size: 16,
                next: None,
            };

            let expected = UsableRegion {
                alloc_region: MemRegion {
                    start: node.start_addr(),
                    size: 16,
                },
                excess_region: None,
            };
            assert_eq!(node.try_allocate(16, 8), Ok(expected));
        }

        #[test_case]
        fn exact_fit_with_new_list_node() {
            let node = ListNode {
                size: 32,
                next: None,
            };

            let expected = UsableRegion {
                alloc_region: MemRegion {
                    start: node.start_addr(),
                    size: 16,
                },
                excess_region: Some(MemRegion {
                    start: node.start_addr() + 16,
                    size: 16, // 32 - 16 = 16 extra bytes (exactly 1 ListNode)
                }),
            };
            assert_eq!(node.try_allocate(16, 8), Ok(expected));
        }

        #[test_case]
        fn fits_with_extra_space() {
            let node = ListNode {
                size: 64,
                next: None,
            };

            let expected = UsableRegion {
                alloc_region: MemRegion {
                    start: node.start_addr(),
                    size: 16,
                },
                excess_region: Some(MemRegion {
                    start: node.start_addr() + 16,
                    size: 48, // 64 - 16 = 48 extra bytes
                }),
            };
            assert_eq!(node.try_allocate(16, 8), Ok(expected));
        }
    }
}

#[cfg(test)]
mod linked_list_allocator {
    use super::test_utils;
    use super::*;

    mod add_free_memory_region {
        use super::*;
        use crate::allocator::align_up;

        #[test_case]
        fn add_first_free_region() {
            let mut test_heap = test_utils::AlignedBuffer::new();

            let mut ll_allocator = LinkedListAllocator::new();
            assert!(ll_allocator.head.next.is_none());

            let aligned_addr = align_up(test_heap.start_addr(), mem::align_of::<ListNode>());

            unsafe {
                ll_allocator.add_free_region(aligned_addr, mem::size_of::<ListNode>());
            }

            let result_node = ll_allocator
                .head
                .next
                .expect("allocator should contain newly inserted memory region");
            assert_eq!(result_node.size, mem::size_of::<ListNode>());
            assert_eq!(result_node.start_addr(), aligned_addr);
            assert!(result_node.next.is_none());
        }

        #[test_case]
        fn add_multiple_free_regions() {
            let mut test_heap = test_utils::AlignedBuffer::new();
            let heap_start = test_heap.start_addr();
            let addrs = [
                heap_start,
                heap_start + 64,
                heap_start + 128,
                heap_start + 192,
                heap_start + 256,
            ];
            let sizes = [32; 5];
            let mut ll_allocator = test_utils::new_ll_allocator_with_five_blocks(addrs, sizes);

            let mut curr = &mut ll_allocator.head;
            for &addr in addrs.iter().rev() {
                let node = curr.next.as_mut().unwrap();
                assert_eq!(node.size, 32);
                assert_eq!(node.start_addr(), addr);
                curr = node;
            }
            assert!(curr.next.is_none());
        }

        // TODO: this is a panicking test, put it in its own test
        // #[test_case]
        // fn unaligned_addr_causes_panic() {
        //     let mut test_heap: AlignedBuffer = AlignedBuffer::new();
        //
        //     let mut ll_allocator = LinkedListAllocator::new();
        //     let addr = test_heap.0.as_mut_ptr() as usize;
        //     let unaligned_addr = addr + 1;
        //     unsafe {
        //         ll_allocator.add_free_region(unaligned_addr, mem::size_of::<ListNode>());
        //     }
        // }

        // TODO: this is a panicking test, put it in its own test
        // #[test_case]
        // fn size_too_small_for_list_node_causes_panic() {
        //     let mut test_heap: AlignedBuffer = AlignedBuffer::new();
        //
        //     let mut ll_allocator = LinkedListAllocator::new();
        //     let addr = test_heap.0.as_mut_ptr() as usize;
        //     let aligned_addr = align_up(addr, mem::align_of::<ListNode>());
        //
        //     unsafe {
        //         ll_allocator.add_free_region(aligned_addr, mem::size_of::<ListNode>() - 1);
        //     }
        // }
    }

    mod extract_first_suitable_region {
        use super::*;

        #[test_case]
        fn empty_allocator() {
            let mut ll_allocator = LinkedListAllocator::new();
            assert!(ll_allocator.extract_first_suitable_region(32, 8).is_none());
        }

        #[test_case]
        fn single_list_node_with_insufficient_space() {
            let mut test_heap = test_utils::AlignedBuffer::new();

            let mut ll_allocator = LinkedListAllocator::new();
            unsafe { ll_allocator.init(test_heap.start_addr(), 4096) };

            assert!(
                ll_allocator
                    .extract_first_suitable_region(8192, 8)
                    .is_none()
            );
        }

        #[test_case]
        fn multiple_list_nodes_with_insufficient_space() {
            let mut test_heap = test_utils::AlignedBuffer::new();
            let heap_start = test_heap.start_addr();
            let addrs = [
                heap_start,
                heap_start + 64,
                heap_start + 128,
                heap_start + 192,
                heap_start + 256,
            ];
            let sizes = [32; 5];
            let mut ll_allocator = test_utils::new_ll_allocator_with_five_blocks(addrs, sizes);

            assert!(
                ll_allocator.extract_first_suitable_region(33, 8).is_none(),
                "requesting 33 bytes when each block is 32 should return None"
            );
        }

        #[test_case]
        fn multiple_list_nodes_one_has_sufficient_space() {
            let mut test_heap = test_utils::AlignedBuffer::new();
            let heap_start = test_heap.start_addr();
            let addrs = [
                heap_start,
                heap_start + 64,
                heap_start + 128,
                heap_start + 192,
                heap_start + 256,
            ];
            let sizes = [32, 32, 32, 64, 32];
            let mut ll_allocator = test_utils::new_ll_allocator_with_five_blocks(addrs, sizes);

            let UsableRegion {
                alloc_region,
                excess_region,
            } = ll_allocator
                .extract_first_suitable_region(48, 8)
                .expect("requesting 33 bytes when each block is 32 should return Some");

            // check that indeed the 4th block was allocated
            assert_eq!(alloc_region.start, addrs[3]);
            assert_eq!(alloc_region.size, 48);

            let excess =
                excess_region.expect("requesting 48 bytes from 64 byte block should have excess");
            assert_eq!(excess.start, addrs[3] + 48);
            assert_eq!(excess.size, 16);
        }
    }
}

#[cfg(test)]
mod alloc_dealloc {
    use super::*;
    use crate::allocator::Locked;

    #[test_case]
    fn empty_allocator_alloc() {
        let locked_allocator = Locked::new(LinkedListAllocator::new());

        let layout = Layout::new::<usize>();
        let ptr = unsafe { locked_allocator.alloc(layout) };

        assert_eq!(ptr, ptr::null_mut());
    }

    #[test_case]
    fn simple_alloc() {
        let mut test_heap = test_utils::AlignedBuffer::new();
        let heap_start = test_heap.start_addr();

        let locked_allocator = Locked::new(LinkedListAllocator::new());

        unsafe { locked_allocator.lock().init(heap_start, 4096) };

        let layout = Layout::new::<usize>();
        let ptr = unsafe { locked_allocator.alloc(layout) };

        assert_eq!(ptr as usize, heap_start);
    }

    #[test_case]
    fn alloc_till_full() {
        let mut test_heap = test_utils::AlignedBuffer::new();
        let heap_start = test_heap.start_addr();

        let locked_allocator = Locked::new(LinkedListAllocator::new());

        unsafe { locked_allocator.lock().init(heap_start, 4096) };

        let layout = Layout::new::<test_utils::TwoKiB>();

        let ptr_1 = unsafe { locked_allocator.alloc(layout) };
        let ptr_2 = unsafe { locked_allocator.alloc(layout) };
        let ptr_3 = unsafe { locked_allocator.alloc(layout) };

        assert_eq!(ptr_1 as usize, heap_start);
        assert_eq!(ptr_2 as usize, heap_start + 2048);
        assert_eq!(ptr_3, ptr::null_mut(), "should run out of heap");
    }

    #[test_case]
    fn dealloced_memory_is_allocable() {
        let mut test_heap = test_utils::AlignedBuffer::new();
        let heap_start = test_heap.start_addr();

        let locked_allocator = Locked::new(LinkedListAllocator::new());

        unsafe { locked_allocator.lock().init(heap_start, 4096) };

        let two_kib_layout = Layout::new::<test_utils::TwoKiB>();

        // Alloc 2 2KiB chunks
        let ptr_1 = unsafe { locked_allocator.alloc(two_kib_layout) };
        let ptr_2 = unsafe { locked_allocator.alloc(two_kib_layout) };
        unsafe { locked_allocator.dealloc(ptr_1, two_kib_layout) };
        unsafe { locked_allocator.dealloc(ptr_2, two_kib_layout) };

        let one_kib_layout = Layout::new::<test_utils::OneKiB>();

        // Reuse the 2 2KiB chunks as 4 1KiB chunks
        let ptr_3 = unsafe { locked_allocator.alloc(one_kib_layout) };
        let ptr_4 = unsafe { locked_allocator.alloc(one_kib_layout) };
        let ptr_5 = unsafe { locked_allocator.alloc(one_kib_layout) };
        let ptr_6 = unsafe { locked_allocator.alloc(one_kib_layout) };

        assert_eq!(ptr_3 as usize, heap_start + 2048);
        assert_eq!(ptr_4 as usize, heap_start + 3072);
        assert_eq!(ptr_5 as usize, heap_start);
        assert_eq!(ptr_6 as usize, heap_start + 1024);
    }
}

#[cfg(test)]
mod test_utils {
    use super::*;
    use crate::allocator::align_up;

    #[repr(align(16))]
    pub struct AlignedBuffer([u8; 4096]);

    impl AlignedBuffer {
        pub fn new() -> Self {
            AlignedBuffer([0; 4096])
        }

        pub fn start_addr(&mut self) -> usize {
            self.0.as_mut_ptr() as usize
        }
    }

    #[allow(dead_code)]
    pub struct OneKiB([u8; 1024]);
    #[allow(dead_code)]
    pub struct TwoKiB([u8; 2048]);

    /// Creates a LinkedListAllocator with 5 blocks, whose starting addresses and sizes are as
    /// specified in the arrays.
    pub fn new_ll_allocator_with_five_blocks(
        addrs: [usize; 5],
        sizes: [usize; 5],
    ) -> LinkedListAllocator {
        let mut ll_allocator = LinkedListAllocator::new();
        for (&addr, size) in addrs.iter().zip(sizes) {
            let aligned_addr = align_up(addr, mem::align_of::<ListNode>());
            unsafe {
                ll_allocator.add_free_region(aligned_addr, size);
            }
        }
        ll_allocator
    }
}
