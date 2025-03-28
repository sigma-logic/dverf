use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Debug, Copy, Clone, FromPrimitive)]
#[repr(u8)]
pub enum BoardId {
	Jellybean = 0,
	Jawbreaker = 1,
	HackrfOneOg = 2,
	Rad10 = 3,
	HackrfOneR9 = 4,
	#[num_enum(default)]
	Unrecognized = 0xFE,
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
#[repr(u8)]
pub enum BoardRev {
	Old = 0,
	R6 = 1,
	R7 = 2,
	R8 = 3,
	R9 = 4,
	R10 = 5,
	GsgR6 = 0x81,
	GsgR7 = 0x82,
	GsgR8 = 0x83,
	GsgR9 = 0x84,
	GsgR10 = 0x85,
	#[num_enum(default)]
	Unrecognized = 0xFE,
}

#[derive(Debug, Copy, Clone, IntoPrimitive)]
#[repr(u16)]
pub enum TransceiverMode {
	Off = 0,
	Receive = 1,
	Transmit = 2,
	Ss = 3,
	CpldUpdate = 4,
	RxSweep = 5,
}

#[derive(Debug)]
#[repr(C)]
pub struct Sample {
	pub i: u8,
	pub q: u8,
}

impl Sample {
	pub fn new(i: u8, q: u8) -> Self {
		Self { i, q }
	}
}
