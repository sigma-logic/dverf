use anyhow::{Context, Result};
use dverf_hackrf::{VENDOR_ID, device::Device};
use futures_lite::future;

fn main() -> Result<()> {
	let device_info = nusb::list_devices()?
		.find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
		.context("device not connected")?;

	let device = device_info.open().context("failed to open device")?;
	let interface = device.claim_interface(0)?;

	let device = Device::from_interface(interface);

	let (id, rev) = future::block_on(future::try_zip(device.board_id(), device.board_rev()))?;

	println!("Board Id: {id:?}. Board Rev: {rev:?}");

	Ok(())
}
