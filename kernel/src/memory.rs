use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::{FlagUpdateError, MapToError, MappedFrame, TranslateResult, UnmapError},
        *,
    },
    PhysAddr, VirtAddr,
};

pub const PAGE_SIZE: usize = Size4KiB::SIZE as usize;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[derive(Debug, Copy, Clone)]
pub struct VirtMemRange(u64, u64);

impl VirtMemRange {
    pub const fn new(start: u64, size: usize) -> VirtMemRange {
        VirtMemRange(start, size as u64)
    }
    pub fn start(&self) -> VirtAddr {
        VirtAddr::new(self.0)
    }
    pub fn stack_start(&self) -> VirtAddr {
        // Stacks grow upward and must be 16-byte aligned.
        VirtAddr::new(self.0 + self.1 - 16)
    }
    pub fn last_addr(&self) -> VirtAddr {
        VirtAddr::new(self.0 + self.1 - 1)
    }
    pub const fn size(&self) -> usize {
        self.1 as usize
    }
}

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
struct BootInfoFrameAllocator {
    memory_regions: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    fn new(memory_regions: &'static MemoryRegions) -> BootInfoFrameAllocator {
        BootInfoFrameAllocator {
            memory_regions,
            next: 0,
        }
    }
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_regions.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

unsafe fn active_level_4_table(phys_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = phys_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    &mut *page_table_ptr // unsafe
}

// TODO secure against stack overflows
// TODO allow heaps to map more memory as needed
pub struct KernelMemory {
    pub privilege_stack: VirtMemRange,
    pub interrupt_stack: VirtMemRange,
    pub double_fault_stack: VirtMemRange,
    heap: VirtMemRange,
}

impl KernelMemory {
    const STACK_SIZE: usize = PAGE_SIZE;
    const HEAP_SIZE: usize = PAGE_SIZE * 8;
    const fn new(base_addr: u64) -> Self {
        let offset = Self::STACK_SIZE as u64;
        KernelMemory {
            privilege_stack: VirtMemRange::new(base_addr, Self::STACK_SIZE),
            interrupt_stack: VirtMemRange::new(base_addr + offset, Self::STACK_SIZE),
            double_fault_stack: VirtMemRange::new(base_addr + (offset * 2), Self::STACK_SIZE),
            heap: VirtMemRange::new(base_addr + (offset * 3), Self::HEAP_SIZE),
        }
    }
    const fn len() -> usize {
        (Self::STACK_SIZE * 3) + Self::HEAP_SIZE
    }
}

pub struct UserMemory {
    pub stack: VirtMemRange,
    heap: VirtMemRange,
}

impl UserMemory {
    const STACK_SIZE: usize = PAGE_SIZE * 4;
    const HEAP_SIZE: usize = PAGE_SIZE * 64;
    const fn new(base_addr: u64) -> Self {
        UserMemory {
            stack: VirtMemRange::new(base_addr, Self::STACK_SIZE),
            heap: VirtMemRange::new(base_addr + (Self::STACK_SIZE as u64), Self::HEAP_SIZE),
        }
    }
}

const EXECUTION_MEMORY_START: u64 = 0xc000_0000_0000;
pub const KERNEL_MEMORY: KernelMemory = KernelMemory::new(EXECUTION_MEMORY_START);
pub const USER_MEMORY: UserMemory =
    UserMemory::new(EXECUTION_MEMORY_START + (KernelMemory::len() as u64));

struct KernelMemoryMapper {
    frame_allocator: BootInfoFrameAllocator,
    mapper: OffsetPageTable<'static>,
    phys_offset: VirtAddr,
}

impl KernelMemoryMapper {
    fn init(
        phys_offset: VirtAddr,
        memory_regions: &'static MemoryRegions,
        memory_layout: KernelMemory,
    ) -> Result<KernelMemoryMapper, MapToError<Size4KiB>> {
        let mapper = unsafe {
            let level_4_table = active_level_4_table(phys_offset);
            OffsetPageTable::new(level_4_table, phys_offset)
        };
        let frame_allocator = BootInfoFrameAllocator::new(memory_regions);

        let mut kernel_mapper = KernelMemoryMapper {
            frame_allocator,
            mapper,
            phys_offset,
        };
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        kernel_mapper.alloc_and_map_range(memory_layout.privilege_stack, flags)?;
        kernel_mapper.alloc_and_map_range(memory_layout.interrupt_stack, flags)?;
        kernel_mapper.alloc_and_map_range(memory_layout.double_fault_stack, flags)?;
        kernel_mapper.alloc_and_map_range(memory_layout.heap, flags)?;
        x86_64::instructions::tlb::flush_all();
        Ok(kernel_mapper)
    }

    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.frame_allocator.allocate_frame()
    }
    unsafe fn map_page(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        self.mapper
            .map_to(page, frame, flags, &mut self.frame_allocator)?
            .ignore();
        Ok(())
    }

    fn alloc_and_map_range(
        &mut self,
        range: VirtMemRange,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        let range_start = Page::from_start_address(range.start()).unwrap();
        let range_end = Page::containing_address(range.last_addr());
        for page in Page::range_inclusive(range_start, range_end) {
            let frame = self
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            unsafe {
                self.map_page(page, frame, flags)?;
            }
        }
        Ok(())
    }
}

pub struct UserMemoryMapper {
    kernel_mapper: &'static mut KernelMemoryMapper,
    allocator: LockedHeap,
}

impl UserMemoryMapper {
    pub fn init(memory_layout: UserMemory) -> Result<UserMemoryMapper, MapToError<Size4KiB>> {
        let kernel_mapper = unsafe {
            KERNEL_MEMORY_MAPPER
                .as_mut()
                .expect("no kernel memory mapper")
        };
        let flags =
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
        kernel_mapper.alloc_and_map_range(memory_layout.stack, flags)?;
        kernel_mapper.alloc_and_map_range(memory_layout.heap, flags)?;
        Ok(UserMemoryMapper {
            kernel_mapper,
            allocator: unsafe {
                LockedHeap::new(
                    memory_layout.heap.start().as_mut_ptr(),
                    memory_layout.heap.size(),
                )
            },
        })
    }

    pub fn phys_offset(&self, phys_addr: PhysAddr) -> VirtAddr {
        VirtAddr::new(phys_addr.as_u64() + self.kernel_mapper.phys_offset.as_u64())
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.kernel_mapper.frame_allocator.allocate_frame()
    }
    pub fn finish_load(&mut self) {
        x86_64::instructions::tlb::flush_all();
    }

    pub fn page_table(&self) -> &OffsetPageTable<'static> {
        &self.kernel_mapper.mapper
    }
    pub fn page_table_mut(&mut self) -> &mut OffsetPageTable<'static> {
        &mut self.kernel_mapper.mapper
    }

    pub unsafe fn map_page(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        mut flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        flags |= PageTableFlags::USER_ACCESSIBLE;
        self.kernel_mapper
            .mapper
            .map_to(page, frame, flags, &mut self.kernel_mapper.frame_allocator)?
            .ignore();
        Ok(())
    }
    pub fn unmap_page(&mut self, page: Page<Size4KiB>) -> Result<(), UnmapError> {
        self.kernel_mapper.mapper.unmap(page)?.1.ignore();
        Ok(())
    }

    pub fn make_range_user_accessible(
        &mut self,
        range: VirtMemRange,
    ) -> Result<(), FlagUpdateError> {
        let range_start = Page::from_start_address(range.start()).unwrap();
        let range_end = Page::containing_address(range.last_addr());
        for page in Page::<Size4KiB>::range_inclusive(range_start, range_end) {
            // Translate the page.
            let res = self.kernel_mapper.mapper.translate(page.start_address());
            let (frame, flags) = match res {
                TranslateResult::Mapped {
                    frame: MappedFrame::Size4KiB(frame),
                    offset: _,
                    flags,
                } => (frame, flags),
                _ => {
                    return Err(FlagUpdateError::PageNotMapped);
                }
            };
            // Remap the page with USER_ACCESSIBLE enabled. This also enables it for parent pages.
            self.unmap_page(page).unwrap();
            unsafe {
                self.map_page(page, frame, flags).unwrap();
            }
        }
        Ok(())
    }
}

static mut KERNEL_MEMORY_MAPPER: Option<KernelMemoryMapper> = None;
static mut USER_MEMORY_MAPPER: Option<UserMemoryMapper> = None;

pub fn init_memory(phys_offset: u64, memory_regions: &'static MemoryRegions) {
    // Create kernel mapper and map kernel heap and interrupt stack.
    let phys_offset = VirtAddr::new(phys_offset);
    let kernel_mapper = KernelMemoryMapper::init(phys_offset, memory_regions, KERNEL_MEMORY)
        .expect("failed to map kernel memory");
    unsafe {
        KERNEL_MEMORY_MAPPER = Some(kernel_mapper);
    }

    // Setup the allocator to use the newly-mapped heap.
    unsafe {
        ALLOCATOR.lock().init(
            KERNEL_MEMORY.heap.start().as_mut_ptr(),
            KERNEL_MEMORY.heap.size(),
        );
    }

    // Map user stack and heap, and create a separate allocator for the user heap.
    let user_mapper = UserMemoryMapper::init(USER_MEMORY).expect("failed to map user memory");
    unsafe {
        USER_MEMORY_MAPPER = Some(user_mapper);
    }
}

pub fn user_memory_mapper() -> &'static mut UserMemoryMapper {
    unsafe { USER_MEMORY_MAPPER.as_mut().expect("no user memory mapper") }
}
pub fn user_allocator() -> &'static LockedHeap {
    &user_memory_mapper().allocator
}
