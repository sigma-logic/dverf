#![doc = include_str!("../readme.md")]

pub mod device;

#[cfg(not(feature = "internals"))]
mod internals;

#[cfg(feature = "internals")]
pub mod internals;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Sample {
	pub i: i8,
	pub q: i8,
}
