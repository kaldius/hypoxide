use core::{
    alloc::{GlobalAlloc, Layout},
    mem, ptr,
};

pub struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    pub const fn new() -> Self {
        ListNode {
            size: 0,
            next: None,
        }
    }

    pub fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    pub fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }

    fn alloc_size(layout: Layout) -> usize {
        layout.size().max(mem::size_of::<Self>())
    }

    fn alloc_align(layout: Layout) -> usize {
        layout
            .align_to(mem::align_of::<Self>())
            .unwrap()
            .pad_to_align()
            .align()
    }

    fn try_allocate(&self, size: usize, align: usize) -> Result<UsableRegion, ()> {
        // FIXME: if aligned up, leading gap will be lost, we ignore for now,
        // but try to think of a solution
        let alloc_start = super::align_up(self.start_addr(), align);
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
    head: ListNode,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: ListNode::new(),
        }
    }

    pub fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // addr must be aligned to ListNode
        assert_eq!(super::align_up(addr, mem::align_of::<ListNode>()), addr);
        // size must be big enough to fit a ListNode
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new();
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
        if let Some(MemRegion { start, size }) = excess_region {
            unsafe {
                allocator.add_free_region(start, size);
            }
        }
        alloc_region.start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
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
