use alloc::collections::BTreeMap;
use bootloader::boot_info::{MemoryRegionKind, MemoryRegions};
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::{MapToError, TranslateError, UnmapError},
        *,
    },
    PhysAddr, VirtAddr,
};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

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

#[derive(Debug, Copy, Clone)]
pub struct VirtMemRange(u64, u64);

impl VirtMemRange {
    const fn new(start: u64, size: usize) -> VirtMemRange {
        VirtMemRange(start, size as u64)
    }
    const fn end_u64(&self) -> u64 {
        self.0 + self.1
    }
    pub fn start(&self) -> VirtAddr {
        VirtAddr::new(self.0)
    }
    pub fn stack_start(&self) -> VirtAddr {
        VirtAddr::new(self.0 + self.1 - 16)
    } // Stacks grow up and must be 16-byte aligned.
    pub fn last_addr(&self) -> VirtAddr {
        VirtAddr::new(self.0 + self.1 - 1)
    }
    pub const fn size(&self) -> usize {
        self.1 as usize
    }
    pub fn size_kib(&self) -> usize {
        self.size() / 1024
    }
}

// TODO secure against stack overflows
// TODO allow heaps to map more memory as needed
const EXECUTION_MEMORY_START: u64 = 0xc000_0000_0000;
pub const KERNEL_STACK_MEMORY: VirtMemRange = VirtMemRange::new(EXECUTION_MEMORY_START, 8 * 1024);
pub const KERNEL_HEAP_MEMORY: VirtMemRange =
    VirtMemRange::new(KERNEL_STACK_MEMORY.end_u64(), 8 * 1024 * 1024);
pub const USER_STACK_MEMORY: VirtMemRange =
    VirtMemRange::new(KERNEL_HEAP_MEMORY.end_u64(), 512 * 1024);
pub const USER_HEAP_MEMORY: VirtMemRange =
    VirtMemRange::new(USER_STACK_MEMORY.end_u64(), 1024 * 1024);

pub trait MemoryMapper {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>>;
    unsafe fn map_page(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>>;
    fn translate_page(&self, page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError>;

    fn alloc_writable_range(&mut self, range: VirtMemRange) -> Result<(), MapToError<Size4KiB>> {
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
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

struct KernelMemoryMapper {
    frame_allocator: BootInfoFrameAllocator,
    mapper: OffsetPageTable<'static>,
    phys_offset: VirtAddr,
}

// not actually Send, but we don't use multithreading :)
unsafe impl Send for KernelMemoryMapper {}

impl KernelMemoryMapper {
    fn init(
        phys_offset: VirtAddr,
        memory_regions: &'static MemoryRegions,
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
        kernel_mapper.alloc_writable_range(KERNEL_STACK_MEMORY)?;
        kernel_mapper.alloc_writable_range(KERNEL_HEAP_MEMORY)?;
        x86_64::instructions::tlb::flush_all();
        Ok(kernel_mapper)
    }
}

impl MemoryMapper for KernelMemoryMapper {
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
    fn translate_page(&self, page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError> {
        self.mapper.translate_page(page)
    }
}

pub struct MemoryContext {
    local_map: BTreeMap<Page<Size4KiB>, (PhysFrame<Size4KiB>, PageTableFlags)>,
    pub allocator: LockedHeap,
}

impl MemoryContext {
    fn new() -> MemoryContext {
        MemoryContext {
            local_map: BTreeMap::new(),
            allocator: LockedHeap::empty(),
        }
    }
}

pub struct UserMemoryMapper {
    kernel_mapper: spin::MutexGuard<'static, KernelMemoryMapper>,
    user_context: MemoryContext,
}

impl UserMemoryMapper {
    pub fn init() -> Result<UserMemoryMapper, MapToError<Size4KiB>> {
        let kernel_mapper = MEMORY_MAPPER.get().unwrap().lock();
        let mut user_mapper = UserMemoryMapper {
            kernel_mapper,
            user_context: MemoryContext::new(),
        };
        user_mapper.alloc_writable_range(USER_STACK_MEMORY)?;
        user_mapper.alloc_writable_range(USER_HEAP_MEMORY)?;
        user_mapper.user_context.allocator = unsafe {
            LockedHeap::new(
                USER_HEAP_MEMORY.start().as_mut_ptr(),
                USER_HEAP_MEMORY.size(),
            )
        };
        Ok(user_mapper)
    }
    pub fn finish_load(self) -> MemoryContext {
        x86_64::instructions::tlb::flush_all();
        self.user_context
    }
    pub fn restore_context(user_context: &MemoryContext) -> Result<(), MapToError<Size4KiB>> {
        let mut kernel_mapper = MEMORY_MAPPER.get().unwrap().lock();
        for (page, (frame, flags)) in user_context.local_map.iter() {
            unsafe {
                kernel_mapper.map_page(*page, *frame, *flags)?;
            }
        }
        Ok(())
    }

    pub fn page_table_mut(&mut self) -> &mut OffsetPageTable<'static> {
        &mut self.kernel_mapper.mapper
    }
    pub fn untranslate(&self, phys_addr: PhysAddr) -> VirtAddr {
        VirtAddr::new(phys_addr.as_u64() + self.kernel_mapper.phys_offset.as_u64())
    }
    pub fn unmap_page(&mut self, page: Page<Size4KiB>) -> Result<(), UnmapError> {
        self.user_context.local_map.remove(&page);
        self.kernel_mapper.mapper.unmap(page)?.1.ignore();
        Ok(())
    }
}

impl MemoryMapper for UserMemoryMapper {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        // TODO track frame allocations so the memory can be reclaimed when the user process quits
        self.kernel_mapper.allocate_frame()
    }
    unsafe fn map_page(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        mut flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        flags |= PageTableFlags::USER_ACCESSIBLE;
        self.user_context.local_map.insert(page, (frame, flags));
        self.kernel_mapper.map_page(page, frame, flags)
    }
    fn translate_page(&self, page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError> {
        self.kernel_mapper.translate_page(page)
    }
}

static MEMORY_MAPPER: spin::Once<spin::Mutex<KernelMemoryMapper>> = spin::Once::new();

pub fn init_memory(phys_offset: u64, memory_regions: &'static MemoryRegions) {
    // Get physical memory offset.
    let phys_offset = VirtAddr::new(phys_offset);
    log::debug!("Physical memory  addr:{:#X}", phys_offset);

    // Create kernel mapper and map kernel heap and interrupt stack.
    MEMORY_MAPPER.call_once(|| {
        let kernel_mapper = KernelMemoryMapper::init(phys_offset, memory_regions).unwrap();
        spin::Mutex::new(kernel_mapper)
    });

    // Setup the allocator to use the newly-mapped heap.
    unsafe {
        ALLOCATOR.lock().init(
            KERNEL_HEAP_MEMORY.start().as_mut_ptr(),
            KERNEL_HEAP_MEMORY.size(),
        );
    }

    // Allocation (Box::new, etc.) is working at this point. Print some numbers.
    log::debug!(
        "Execution memory addr:{:#X}",
        VirtAddr::new(EXECUTION_MEMORY_START)
    );
    log::debug!(
        "  kernel stack size:{}KiB\n  kernel heap  size:{}KiB",
        KERNEL_STACK_MEMORY.size_kib(),
        KERNEL_HEAP_MEMORY.size_kib()
    );
    log::debug!(
        "  user stack size:{}KiB\n  user heap  size:{}KiB",
        USER_STACK_MEMORY.size_kib(),
        USER_HEAP_MEMORY.size_kib()
    );
}
