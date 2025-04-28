use std::time::Instant;

use anyhow::{Context, Result};
use dverf::{Device, TransceiverMode, VENDOR_ID};
use futures::{StreamExt, executor::block_on};

fn main() -> Result<()> {
	let device_info = nusb::list_devices()?
		.find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
		.context("device not connected")?;

	let device = device_info.open().context("failed to open device")?;
	let interface = device.claim_interface(0)?;

	let mut device = Device::from_interface(interface);

	block_on(async {
		device.set_freq(2412, 0).await?;
		device.set_sample_rate(200_000_000, 10).await?;
		device.set_baseband_filter_bandwidth(15_000_000).await?;
		device.set_lna_gain(40).await?;
		device.set_vga_gain(8).await?;
		device.set_transceiver_mode(TransceiverMode::Receive).await?;

		let mut samples_received = 0;

		let inst = Instant::now();

		while samples_received < 20_000_000 {
			let chunk = match device.next().await {
				Some(Ok(it)) => it,
				Some(Err(err)) => return Err(err.into()),
				None => unreachable!(),
			};

			samples_received += chunk.len();
			// Do something with chunks
		}

		let elapsed = inst.elapsed();

		println!("{} samples received in {}ms", samples_received, elapsed.as_millis());

		device.reset().await?;

		Result::<()>::Ok(())
	})?;

	Ok(())
}
