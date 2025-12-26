use core::{
    alloc::{GlobalAlloc, Layout},
    mem, ptr,
};

use crate::mutex::Mutex;

use super::align_up;

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub(crate) struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    pub(crate) const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    pub(crate) unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.add_free_region(heap_start, heap_size) };
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // ensure that the freed region is capable of holding ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // before: head -> node_0 -> ...
        // after: head -> new_node -> node_0 -> ...

        // create a new list node and append it at the start of the list
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr);
        }
    }

    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // reset of region too small to hold a ListNode
            // (required because the allocation splits the region in a used and a free part)
            return Err(());
        }

        Ok(alloc_start)
    }

    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        // reference to current list node, updated for each iteration
        let mut current_node = &mut self.head;

        // looks for a large enough memory region in linked list
        while let Some(ref mut region) = current_node.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current_node.next.take().unwrap(), alloc_start));
                current_node.next = next;
                return ret;
            } else {
                current_node = current_node.next.as_mut().unwrap();
            }
        }

        None
    }

    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `ListNode`.
    ///
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();

        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Mutex<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // perform layout adjustment
        let (size, align) = LinkedListAllocator::size_align(layout);
        // lock the allocator
        let mut allocator = self.lock();

        // find suitable free region
        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            // if a suitable region is found, the region is splited to allocated region and free region.
            // | suitable region | -> | allocated_region | splited_free_region |
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    // the splited free region is added to the linked list
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);
        unsafe {
            self.lock().add_free_region(ptr as usize, size);
        }
    }
}
