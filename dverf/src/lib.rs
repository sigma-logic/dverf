#![doc = include_str!("../readme.md")]

use std::{
	cmp::Ordering,
	collections::VecDeque,
	fmt::{Debug, Display, Formatter},
	mem,
	ops::{Deref, DerefMut},
	pin::Pin,
	task::{Context, Poll, ready},
};

use futures::{Sink, Stream};
use num::Complex;
use num_enum::{FromPrimitive, IntoPrimitive};
pub use nusb;
use nusb::transfer::{Completion, ControlIn, ControlOut, ControlType, Queue, Recipient, RequestBuffer};
use thiserror::Error;

pub mod internals {
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
	/// In case the length of a slice is odd
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
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct Sample(Complex<i8>);

impl Deref for Sample {
	type Target = Complex<i8>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Sample {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl From<Sample> for Complex<i8> {
	fn from(value: Sample) -> Self {
		value.0
	}
}

impl From<Complex<i8>> for Sample {
	fn from(value: Complex<i8>) -> Self {
		Self(value)
	}
}

/// `vendor_id` is constant for all variations of HackRF
pub const VENDOR_ID: u16 = 0x1d50;

pub type Result<T = (), E = Error> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Transfer: {0}")]
	Transfer(#[from] nusb::transfer::TransferError),

	#[error("Invalid response")]
	Resp,

	#[error("Invalid value for parameter: {0}")]
	Param(&'static str),
}

/// Main interface to a HackRF One device
///
/// Handles USB communication and provides methods for device configuration and
/// control.
///
/// Implements [`Stream`] and [`Sink`] for receiving and transmitting IQ samples
/// asynchronously, respectively.
pub struct Device {
	usb_if: nusb::Interface,
	bulk_in: Queue<RequestBuffer>,
	bulk_out: Queue<Vec<u8>>,

	current_buffer: Vec<u8>,
	free_buffers: Vec<Vec<u8>>,
	pending_buffers: VecDeque<Vec<u8>>,
}

impl Device {
	/// Creates a Device from a pre-opened USB interface.
	///
	/// We do not provide direct connectivity for the following reasons:
	/// * We don't know for sure what the `product_id` of your device is
	/// * We don't know what else you're going to do with this usb device
	/// * That takes too much responsibility for such a lightweight crate.
	///
	/// But in most cases, it's as simple as the [nusb](https://docs.rs/nusb/0.1.13/nusb/struct.Device.html) example
	///
	/// # Examples
	/// ```no_run
	/// use dverf::VENDOR_ID;
	///
	/// let device_info = nusb::list_devices()?
	///     .find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
	///     .context("device not connected")?;
	///
	/// let device = device_info.open().context("failed to open device")?;
	/// let interface = device.claim_interface(0)?;
	/// ```
	pub fn from_interface(usb_if: nusb::Interface) -> Self {
		Self {
			bulk_in: usb_if.bulk_in_queue(0x81),
			bulk_out: usb_if.bulk_out_queue(0x02),
			usb_if,

			current_buffer: Vec::with_capacity(TRANSFER_SIZE),
			free_buffers: Vec::with_capacity(TRANSFER_COUNT),

			pending_buffers: VecDeque::new(),
		}
	}

	/// Extracts the underlying USB interface. Underlying bulk queues will be
	/// dropped cancelling incomplete transfers
	pub fn into_interface(self) -> nusb::Interface {
		self.usb_if
	}

	/// Resets the HackRF device to its initial state
	pub async fn reset(&self) -> Result<()> {
		control_out(&self.usb_if, VendorRequest::Reset, 0, 0, &[]).await
	}

	/// Reads the board identifier
	pub async fn board_id(&self) -> Result<BoardId> {
		let res = control_in(&self.usb_if, VendorRequest::BoardIdRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}

	/// Reads the board revision
	pub async fn board_rev(&self) -> Result<BoardRev> {
		let res = control_in(&self.usb_if, VendorRequest::BoardRevRead, 0, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		Ok(first.into())
	}

	/// Reads the board version string
	pub async fn version(&self) -> Result<String> {
		let nullstr = control_in(&self.usb_if, VendorRequest::VersionStringRead, 0, 0, 255).await?;
		Ok(String::from_utf8_lossy(&nullstr).into_owned())
	}

	/// Sets the transceiver operating mode.
	///
	/// Always set to either
	/// [`TransceiverMode::Receive`] or [`TransceiverMode::Transmit`] before using
	/// [`Stream`] or [`Sink`] trait implementations
	pub async fn set_transceiver_mode(&self, mode: TransceiverMode) -> Result<()> {
		control_in(&self.usb_if, VendorRequest::SetTransceiverMode, 0, mode.into(), 0).await.and(Ok(()))
	}

	/// Sets the center RF frequency
	/// Note that target frequency split into integer MHz and Hz parts
	///
	/// Examples
	/// * `446` and `6875` to tune to `446.06875 Mhz`.
	/// * `2400` and `0` to tune to `2.4 Ghz`
	pub async fn set_freq(&self, mhz: u32, hz: u32) -> Result<()> {
		let mut data = [0u8; 8];
		let (mhz_le, hz_le) = (mhz.to_le_bytes(), hz.to_le_bytes());
		data[..4].copy_from_slice(&mhz_le);
		data[4..].copy_from_slice(&hz_le);

		control_out(&self.usb_if, VendorRequest::SetFreq, 0, 0, &data).await?;

		Ok(())
	}

	/// Sets the baseband filter bandwidth. Should be less than sample rate to
	/// reduce aliasing
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

	/// Sets the sample rate
	///
	/// # Note
	/// Actual sample rate is `hz / divider`
	///
	/// # Example
	/// For 20 Mhz sample rate `hz = 200_000_000`, `divider = 10`
	pub async fn set_sample_rate(&self, hz: u32, divider: u32) -> Result<()> {
		let (hz_le, divider_le) = (hz.to_le_bytes(), divider.to_le_bytes());
		let mut data = [0u8; 8];
		data[..4].copy_from_slice(&hz_le);
		data[4..].copy_from_slice(&divider_le);

		control_out(&self.usb_if, VendorRequest::SetSampleRate, 0, 0, &data).await
	}

	/// Sets LNA (Low-Noise Amplifier) gain (0-40 dB in 8 dB steps).
	/// Set to maximum value (40 dB) unless you encounter overload
	pub async fn set_lna_gain(&self, value: u16) -> Result<()> {
		let res = control_in(&self.usb_if, VendorRequest::SetLnaGain, value, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		if first == 1 { Ok(()) } else { Err(Error::Param("Lna Gain")) }
	}

	/// Sets VGA (Variable Gain Amplifier) gain (0-62 dB in 2 dB steps).
	/// Set as less as possible, it amplifies noise
	pub async fn set_vga_gain(&self, value: u16) -> Result<()> {
		let res = control_in(&self.usb_if, VendorRequest::SetVgaGain, value, 0, 1).await?;

		let Some(&first) = res.first() else {
			return Err(Error::Resp);
		};

		if first == 1 { Ok(()) } else { Err(Error::Param("Vga Gain")) }
	}

	/// On/Off transmitter amplifier
	pub async fn amp_enable(&self, enable: bool) -> Result<()> {
		control_out(&self.usb_if, VendorRequest::AmpEnable, 0, enable.into(), &[]).await
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

impl Display for BoardId {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			BoardId::Jellybean => write!(f, "Jellybean"),
			BoardId::Jawbreaker => write!(f, "Jawbreaker"),
			BoardId::HackrfOneOg => write!(f, "HackRF One Og"),
			BoardId::Rad10 => write!(f, "Rad 10"),
			BoardId::HackrfOneR9 => write!(f, "HackRF One R9"),
			BoardId::Unrecognized => write!(f, "Unrecognized"),
		}
	}
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

impl Display for BoardRev {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			BoardRev::Old => write!(f, "Old"),
			BoardRev::R6 => write!(f, "R6"),
			BoardRev::R7 => write!(f, "R7"),
			BoardRev::R8 => write!(f, "R8"),
			BoardRev::R9 => write!(f, "R9"),
			BoardRev::R10 => write!(f, "R10"),
			BoardRev::GsgR6 => write!(f, "GSG R6"),
			BoardRev::GsgR7 => write!(f, "GSG R7"),
			BoardRev::GsgR8 => write!(f, "GSG R8"),
			BoardRev::GsgR9 => write!(f, "GSG R9"),
			BoardRev::GsgR10 => write!(f, "GSG R10"),
			BoardRev::Unrecognized => write!(f, "Unrecognized"),
		}
	}
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

pub const TRANSFER_COUNT: usize = 4;
pub const TRANSFER_SIZE: usize = 262_144;

impl Device {
	/// Reinforces the type when using [`futures::SinkExt`] to avoid
	/// full-qualified path
	pub fn as_sink<T: AsRef<[Sample]>>(&mut self) -> &mut (impl Sink<T, Error = Error> + Unpin) {
		self
	}

	fn poll_next(&mut self, cx: &mut Context) -> Poll<Option<Result<Vec<Sample>>>> {
		while self.bulk_in.pending() < TRANSFER_COUNT {
			self.bulk_in.submit(RequestBuffer::new(TRANSFER_SIZE));
		}

		let mut completion = ready!(self.bulk_in.poll_next(cx));

		if let Err(err) = completion.status {
			return Poll::Ready(Some(Err(err.into())));
		}

		let data = mem::replace(&mut completion.data, Vec::with_capacity(TRANSFER_SIZE));

		self.bulk_in.submit(RequestBuffer::reuse(completion.data, TRANSFER_SIZE));

		let samples = internals::bytes_into_samples(data);

		Poll::Ready(Some(Ok(samples)))
	}

	#[inline]
	fn submit_or_enqueue(&mut self) {
		let replace_buffer = self.free_buffers.pop().unwrap_or_else(|| Vec::with_capacity(TRANSFER_SIZE));
		let buf = mem::replace(&mut self.current_buffer, replace_buffer);

		if self.bulk_out.pending() == TRANSFER_COUNT {
			self.pending_buffers.push_back(buf);
		} else {
			self.bulk_out.submit(buf);
		}
	}

	fn start_send<T: AsRef<[Sample]>>(&mut self, chunk: T) {
		let bytes = internals::samples_as_bytes(chunk.as_ref());

		assert!(self.bulk_out.pending() < TRANSFER_COUNT, "Sink is not ready to accept new items");

		let required = TRANSFER_SIZE - self.current_buffer.len();

		match bytes.len().cmp(&required) {
			Ordering::Less => {
				self.current_buffer.extend(bytes);
			}
			Ordering::Equal => {
				self.current_buffer.extend(bytes);
				self.submit_or_enqueue();
			}
			Ordering::Greater => {
				for &byte in bytes {
					self.current_buffer.push(byte);

					if self.current_buffer.len() == TRANSFER_SIZE {
						self.submit_or_enqueue();
					}
				}
			}
		};
	}

	fn poll_completion(&mut self, cx: &mut Context) -> Poll<Result<()>> {
		if self.bulk_out.pending() == 0 {
			return Poll::Ready(Ok(()));
		}

		let Completion { data, status } = ready!(self.bulk_out.poll_next(cx));
		status?;

		self.free_buffers.push(data.reuse());

		if let Some(buf) = self.pending_buffers.pop_front() {
			self.bulk_out.submit(buf);
			cx.waker().wake_by_ref();
			Poll::Pending
		} else {
			Poll::Ready(Ok(()))
		}
	}

	fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<()>> {
		if self.bulk_out.pending() < TRANSFER_COUNT {
			return Poll::Ready(Ok(()));
		}

		Self::poll_completion(self, cx)
	}

	fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<()>> {
		if !self.current_buffer.is_empty() && self.current_buffer.len() < TRANSFER_SIZE {
			self.submit_or_enqueue();
		}

		self.poll_completion(cx)
	}
}

impl Stream for Device {
	type Item = Result<Vec<Sample>>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		Self::poll_next(&mut self, cx)
	}
}

impl<T: AsRef<[Sample]>> Sink<T> for Device {
	type Error = Error;

	fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Self::poll_ready(&mut self, cx)
	}

	fn start_send(mut self: Pin<&mut Self>, chunk: T) -> Result<(), Self::Error> {
		Self::start_send(&mut self, chunk);
		Ok(())
	}

	fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Self::poll_flush(&mut self, cx)
	}

	fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		panic!("Instead of closing the sink, just drop or destruct `Device`");
	}
}

impl Debug for Device {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "Device {{ .. }}")
	}
}
