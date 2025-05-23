use anyhow::{Context, Result};
use dverf::{Device, VENDOR_ID};
use futures::executor::block_on;
use futures_concurrency::future::TryJoin;

fn main() -> Result<()> {
	let device_info = nusb::list_devices()?
		.find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
		.context("device not connected")?;

	let device = device_info.open().context("failed to open device")?;
	let interface = device.claim_interface(0)?;

	let device = Device::from_interface(interface);

	let (id, rev, version) = block_on((device.board_id(), device.board_rev(), device.version()).try_join())?;

	println!("Id: {id:?}");
	println!("Rev: {rev:?}");
	println!("Version: {version}");

	Ok(())
}
