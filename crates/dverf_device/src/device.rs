use std::{
	fmt::{Debug, Formatter},
	mem,
	mem::ManuallyDrop,
	pin::Pin,
	task::{Context, Poll},
};

use futures_lite::{Stream, ready};
use num_enum::IntoPrimitive;
use nusb::transfer::{ControlIn, ControlOut, ControlType, Queue, Recipient, RequestBuffer};

use crate::{
	BoardId, BoardRev, Error, Result,
	consts::{TRANSFER_SIZE, TRANSFERS_NUM},
	ty,
	ty::Sample,
};

pub struct Device {
	usb_if: nusb::Interface,
	bulk_in: Queue<RequestBuffer>,
	bulk_out: Queue<Vec<u8>>,
}

impl Debug for Device {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Device")
			.field("usb_if", &format_args!("Interface {{ .. }}"))
			.field("bulk_in", &format_args!("Queue::<RequestBuffer> {{ .. }}"))
			.field("bulk_out", &format_args!("Queue::<Vec<u8>> {{ .. }}"))
			.finish()
	}
}

impl Device {
	pub fn from_interface(usb_if: nusb::Interface) -> Self {
		Self {
			bulk_in: usb_if.bulk_in_queue(0x81),
			bulk_out: usb_if.bulk_out_queue(0x02),
			usb_if,
		}
	}

	pub fn into_interface(self) -> nusb::Interface {
		self.usb_if
	}

	pub async fn reset(&self) -> Result<()> {
		control_out(&self.usb_if, VendorRequest::Reset, 0, 0, &[]).await
	}

	pub async fn board_id(&self) -> Result<BoardId> {
		let res = control_in(&self.usb_if, VendorRequest::BoardIdRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}

	pub async fn board_rev(&self) -> Result<BoardRev> {
		let res = control_in(&self.usb_if, VendorRequest::BoardRevRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}

	pub async fn version(&self) -> Result<String> {
		let nullstr = control_in(&self.usb_if, VendorRequest::VersionStringRead, 0, 0, 255).await?;
		Ok(String::from_utf8_lossy(&nullstr).into_owned())
	}

	pub async fn set_transceiver_mode(&self, mode: ty::TransceiverMode) -> Result<()> {
		control_in(&self.usb_if, VendorRequest::SetTransceiverMode, 0, mode.into(), 0).await.and(Ok(()))
	}

	pub async fn set_freq(&self, mhz: u32, hz: u32) -> Result<()> {
		let mut data = [0u8; 8];
		let (mhz_le, hz_le) = (mhz.to_le_bytes(), hz.to_le_bytes());
		data[..4].copy_from_slice(&mhz_le);
		data[4..].copy_from_slice(&hz_le);

		control_out(&self.usb_if, VendorRequest::SetFreq, 0, 0, &data).await?;

		Ok(())
	}

	pub async fn set_baseband_filter_bandwidth(&self, hz: u32) -> Result<()> {
		control_out(
			&self.usb_if,
			VendorRequest::SetBasebandFilterBandwidth,
			(hz >> 16) as u16,
			(hz & 0xffff) as u16,
			&[],
		)
		.await
	}

	pub async fn set_sample_rate(&self, hz: u32, divider: u32) -> Result<()> {
		let (hz_le, divider_le) = (hz.to_le_bytes(), divider.to_le_bytes());
		let mut data = [0u8; 8];
		data[..4].copy_from_slice(&hz_le);
		data[4..].copy_from_slice(&divider_le);

		control_out(&self.usb_if, VendorRequest::SetSampleRate, 0, 0, &data).await
	}

	pub async fn set_lna_gain(&self, value: u16) -> Result<()> {
		let res = control_in(&self.usb_if, VendorRequest::SetLnaGain, value, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		if first != 1 { Err(Error::Param("Lna Gain")) } else { Ok(()) }
	}

	pub async fn set_vga_gain(&self, value: u16) -> Result<()> {
		let res = control_in(&self.usb_if, VendorRequest::SetVgaGain, value, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		if first != 1 { Err(Error::Param("Vga Gain")) } else { Ok(()) }
	}
}

async fn control_in(interface: &nusb::Interface, vrid: VendorRequest, index: u16, value: u16, length: u16) -> Result<Vec<u8>> {
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

async fn control_out(interface: &nusb::Interface, vrid: VendorRequest, index: u16, value: u16, data: &[u8]) -> Result<()> {
	let data = ControlOut {
		control_type: ControlType::Vendor,
		recipient: Recipient::Device,
		request: vrid.into(),
		index,
		value,
		data,
	};

	let result = interface.control_out(data).await;
	result.status?;
	Ok(())
}

impl Device {
	pub fn poll_next(&mut self, cx: &mut Context) -> Poll<Option<Result<Vec<ty::Sample>>>> {
		while self.bulk_in.pending() < TRANSFERS_NUM {
			self.bulk_in.submit(RequestBuffer::new(TRANSFER_SIZE));
		}

		let mut completion = ready!(self.bulk_in.poll_next(cx));

		if let Err(err) = completion.status {
			return Poll::Ready(Some(Err(err.into())));
		}

		let data = mem::replace(&mut completion.data, Vec::with_capacity(TRANSFER_SIZE));
		let mut data = ManuallyDrop::new(data);

		let len = data.len() / 2;
		let cap = data.capacity() / 2;
		let ptr = data.as_mut_ptr() as *mut Sample;

		let chunk = unsafe { Vec::from_raw_parts(ptr, len, cap) };

		self.bulk_in.submit(RequestBuffer::reuse(completion.data, TRANSFER_SIZE));

		Poll::Ready(Some(Ok(chunk)))
	}
}

impl Stream for Device {
	type Item = Result<Vec<Sample>>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		Self::poll_next(&mut self, cx)
	}
}

#[allow(unused)]
#[derive(Debug, Copy, Clone, IntoPrimitive)]
#[repr(u8)]
enum VendorRequest {
	SetTransceiverMode = 1,
	Max2837Write = 2,
	Max2837Read = 3,
	Si5351CWrite = 4,
	Si5351CRead = 5,
	SetSampleRate = 6,
	SetBasebandFilterBandwidth = 7,
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
