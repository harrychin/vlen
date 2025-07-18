//! Shared helpers for vlen (pointer utilities)

/// Returns a pointer to a reference, useful for unsafe operations.
#[inline]
pub const fn ptr_from_ref<T>(r: &T) -> *const T {
	r as *const T
}

/// Returns a pointer to a mutable reference, useful for unsafe operations.
#[inline]
pub const fn ptr_from_mut<T>(r: &mut T) -> *mut T {
	r as *mut T
}

/// Checks if a pointer is aligned to the specified alignment.
#[inline(always)]
pub fn is_aligned<const ALIGN: usize>(ptr: *const u8) -> bool {
	(ptr as usize & (ALIGN - 1)) == 0
}
