use std::env;

use anyhow::Context;
use async_executor::Executor;
use async_fs::File;
use dverf::{
	BytesToSamples, SamplesAsBytes,
	device::{Device, TRANSFER_SIZE, TransceiverMode, VENDOR_ID},
};
use futures::{AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt, io::BufReader};
use shared::example;

example! {
	async fn main(mut device: Device, _ex: &Executor<'_>) {
		device.set_freq(5658, 0).await?;
		device.set_sample_rate(180_000_000, 10).await?;
		device.set_baseband_filter_bandwidth(17_000_000).await?;

		let action = env::args().nth(1).unwrap_or("play".into());

		if action == "play" {
			let mut file = BufReader::new(File::open("samples.iq").await?);

			let mut buf = vec![0u8; TRANSFER_SIZE / 2];

			device.set_transceiver_mode(TransceiverMode::Transmit).await?;
			device.amp_enable(true).await?;

			let sink = device.as_sink();

			while file.read_exact(&mut buf).await.is_ok() {
				sink.feed(buf.to_samples()).await?;
			}

			sink.flush().await?;

			println!("Samples replayed");
		}

		if action == "record" {
			let mut samples = Vec::new();

			device.set_lna_gain(40).await?;
			device.set_vga_gain(12).await?;
			device.set_transceiver_mode(TransceiverMode::Receive).await?;

			let mut received = 0;

			while received < 200_000_000 {
				let chunk = match device.next().await {
					Some(Ok(it)) => it,
					Some(Err(err)) => return Err(err.into()),
					None => unreachable!(),
				};

				received += chunk.len();
				samples.extend(chunk);
			}

			let mut file = File::create("samples.iq").await?;

			file.write_all(samples.as_bytes()).await?;

			println!("Samples recorded");
		}

		device.set_transceiver_mode(TransceiverMode::Off).await?;

		Ok(())
	}
}
