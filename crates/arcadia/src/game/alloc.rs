#[skyline::from_offset(crate::offsets::je_aligned_alloc())]
pub(crate) unsafe fn je_aligned_alloc(alignment: usize, size: usize) -> *mut u8;

#[skyline::from_offset(crate::offsets::free_default())]
pub(crate) unsafe fn free_default(block: *mut u8);
