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

/// Copies `len` bytes from `src` to `dst` starting at `dst_offset`.
///
/// # Safety
/// Caller must ensure bounds are valid.
#[inline]
pub const fn const_copy_slice(
	src: &[u8],
	dst: &mut [u8],
	dst_offset: usize,
	len: usize,
) {
	let mut i = 0;
	while i < len {
		dst[dst_offset + i] = src[i];
		i += 1;
	}
}

/// Reads an array of size `N` from `src` starting at `offset`.
#[inline]
pub const fn const_read_array<const N: usize>(
	src: &[u8],
	offset: usize,
) -> [u8; N] {
	let mut arr = [0u8; N];
	let mut i = 0;
	while i < N {
		arr[i] = src[offset + i];
		i += 1;
	}
	arr
}
