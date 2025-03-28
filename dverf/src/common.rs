use std::{mem::ManuallyDrop, slice};

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Sample {
	pub i: u8,
	pub q: u8,
}

impl Sample {
	pub fn new(i: u8, q: u8) -> Self {
		Self { i, q }
	}

	pub fn zero() -> Self {
		Self { i: 0, q: 0 }
	}
}

pub trait BytesAsSamples {
	fn as_samples(&self) -> &[Sample];
}

impl<T: AsRef<[u8]>> BytesAsSamples for T {
	#[inline]
	fn as_samples(&self) -> &[Sample] {
		let slice = self.as_ref();
		let len = slice.len();

		assert_eq!(len & 1, 0, "Slice length must be multiply of 2");

		let ptr = slice.as_ptr().cast::<Sample>();

		unsafe { slice::from_raw_parts(ptr, len / 2) }
	}
}

pub trait BytesIntoSamples {
	fn into_samples(self) -> Vec<Sample>;
}

impl BytesIntoSamples for Vec<u8> {
	#[inline]
	fn into_samples(self) -> Vec<Sample> {
		let mut this = ManuallyDrop::new(self);
		let len = this.len();
		let cap = this.capacity();

		assert_eq!(len & 1, 0, "Vec length must be multiply of 2");

		let ptr = this.as_mut_ptr().cast::<Sample>();

		unsafe { Vec::from_raw_parts(ptr, len / 2, cap / 2) }
	}
}

pub trait BytesToSamples {
	fn to_samples(&self) -> Vec<Sample>;
}

impl<T: AsRef<[u8]>> BytesToSamples for T {
	#[inline]
	fn to_samples(&self) -> Vec<Sample> {
		self.as_ref().to_owned().into_samples()
	}
}

pub trait SamplesAsBytes {
	fn as_bytes(&self) -> &[u8];
}

impl<T: AsRef<[Sample]>> SamplesAsBytes for T {
	#[inline]
	fn as_bytes(&self) -> &[u8] {
		let slice = self.as_ref();
		let len = slice.len();
		let ptr = slice.as_ptr().cast::<u8>();

		unsafe { slice::from_raw_parts(ptr, len * 2) }
	}
}
