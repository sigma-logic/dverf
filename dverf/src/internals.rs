use std::{mem::ManuallyDrop, slice};

use crate::Sample;

#[inline]
pub fn samples_as_bytes(slice: &[Sample]) -> &[u8] {
	let len = slice.len();
	let ptr = slice.as_ptr().cast::<u8>();

	unsafe { slice::from_raw_parts(ptr, len * 2) }
}

/// # Panics
/// In case the length of vec is odd
#[inline]
pub fn bytes_into_samples(vec: Vec<u8>) -> Vec<Sample> {
	assert_eq!(vec.len() & 1, 0, "Vec length must be multiply of 2");

	let mut vec = ManuallyDrop::new(vec);

	let len = vec.len() / 2;
	let cap = vec.capacity() / 2;
	let ptr = vec.as_mut_ptr().cast::<Sample>();

	unsafe { Vec::from_raw_parts(ptr, len, cap) }
}

/// # Panics
/// In case the length of slice is odd
#[inline]
pub fn bytes_as_samples(slice: &[u8]) -> &[Sample] {
	assert_eq!(slice.len() & 1, 0, "Slice length must be multiply of 2");

	let ptr = slice.as_ptr().cast::<Sample>();
	let len = slice.len() / 2;

	unsafe { slice::from_raw_parts(ptr, len) }
}

#[inline]
pub fn bytes_to_samples(slice: &[u8]) -> Vec<Sample> {
	let samples = bytes_as_samples(slice);
	samples.to_owned()
}
