use std::env;

use anyhow::Context;
use async_executor::Executor;
use async_fs::File;
use dverf::{Device, TRANSFER_SIZE, TransceiverMode, VENDOR_ID, internals};
use futures::{AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt, io::BufReader};
use shared::example;

example! {
	async fn main(mut device: Device, _ex: &Executor<'_>) {
		device.set_freq(5658, 0).await?;
		device.set_sample_rate(180_000_000, 10).await?;
		device.set_baseband_filter_bandwidth(17_734_475).await?;

		let action = env::args().nth(1).unwrap_or("play".into());

		if action == "play" {
			let mut file = BufReader::new(File::open("samples.iq").await?);

			let mut buf = vec![0u8; TRANSFER_SIZE / 2];

			device.amp_enable(true).await?;
			device.set_transceiver_mode(TransceiverMode::Transmit).await?;

			let sink = device.as_sink();

			while file.read_exact(&mut buf).await.is_ok() {
				sink.feed(internals::bytes_to_samples(&buf)).await?;
			}

			sink.flush().await?;

			println!("Samples replayed");
		}

		if action == "record" {
			let mut samples = Vec::new();

			device.set_lna_gain(40).await?;
			device.set_vga_gain(24).await?;
			device.set_transceiver_mode(TransceiverMode::Receive).await?;

			let mut received = 0;

			while received < 100_000_000 {
				let chunk = match device.next().await {
					Some(Ok(it)) => it,
					Some(Err(err)) => return Err(err.into()),
					None => unreachable!(),
				};

				received += chunk.len();
				samples.extend(chunk);
			}

			let mut file = File::create("samples.iq").await?;

			file.write_all(internals::samples_as_bytes(&samples)).await?;

			println!("Samples recorded");
		}

		device.set_transceiver_mode(TransceiverMode::Off).await?;

		Ok(())
	}
}
