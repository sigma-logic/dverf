use std::fmt::{Debug, Formatter};

use num_enum::{FromPrimitive, IntoPrimitive};
use nusb::transfer::{ControlIn, ControlType, Recipient};

use crate::{Error, Result};

pub struct Device {
	usb_if: nusb::Interface,
}

impl Debug for Device {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Device {{ .. }}")
	}
}

impl Device {
	pub fn from_interface(usb_if: nusb::Interface) -> Self {
		Self { usb_if }
	}

	pub fn into_interface(self) -> nusb::Interface {
		self.usb_if
	}

	pub async fn board_id(&self) -> Result<BoardId> {
		let res = request(&self.usb_if, VendorRequest::BoardIdRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}

	pub async fn board_rev(&self) -> Result<BoardRev> {
		let res = request(&self.usb_if, VendorRequest::BoardRevRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}
}

async fn request(
	interface: &nusb::Interface,
	vrid: VendorRequest,
	index: u16,
	value: u16,
	length: u16,
) -> Result<Vec<u8>> {
	let data = ControlIn {
		control_type: ControlType::Vendor,
		recipient: Recipient::Device,
		request: vrid.into(),
		index,
		value,
		length,
	};

	let result = interface.control_in(data).await;
	result.status?;
	Ok(result.data)
}

#[derive(Debug, Copy, Clone, IntoPrimitive)]
#[repr(u8)]
enum VendorRequest {
	SetTransceiverMode = 1,
	Max2837Write = 2,
	Max2837Read = 3,
	Si5351CWrite = 4,
	Si5351CRead = 5,
	SampleRateSet = 6,
	BasebandFilterBandwidthSet = 7,
	Rffc5071Write = 8,
	Rffc5071Read = 9,
	SpiFlashErase = 10,
	SpiFlashWrite = 11,
	SpiFlashRead = 12,
	BoardIdRead = 14,
	VersionStringRead = 15,
	SetFreq = 16,
	AmpEnable = 17,
	BoardPartIdSerialNoRead = 18,
	SetLnaGain = 19,
	SetVgaGain = 20,
	SetTxVgaGain = 21,
	AntennaEnable = 23,
	SetFreqExplicit = 24,
	UsbWcidVendorReq = 25,
	InitSweep = 26,
	OperaCakeGetBoards = 27,
	OperaCakeSetPorts = 28,
	SetHwSyncMode = 29,
	Reset = 30,
	OperaCakeSetRanges = 31,
	ClkOutEnable = 32,
	SpiFlashStatus = 33,
	SpiFlashClearStatus = 34,
	OperaCakeGpioTest = 35,
	CpldChecksum = 36,
	UiEnable = 37,
	OperaCakeSetMode = 38,
	OperaCakeGetMode = 39,
	OperaCakeSetDwellTimes = 40,
	GetM0State = 41,
	SetTxUnderrunLimit = 42,
	SetRxOverrunLimit = 43,
	GetClkinStatus = 44,
	BoardRevRead = 45,
	SupportedPlatformRead = 46,
	SetLeds = 47,
	SetUserBiasTOpts = 48,
}

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
