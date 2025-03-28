# Dverf

Powerful SDR toolkit designed in pure Rust for HackRF One

### Supported and tested devices

* [OpenSourceSDRLab R10C](https://opensourcesdrlab.com/products/r10c-hrf-sdr-software-defined-1mhz-to-6ghz-mainboard-development-board-kit)

## Examples

### Open device

Open usb device as you like with [nusb](https://docs.rs/nusb/) crate

```rust
use dverf::device::{Device, VENDOR_ID};

fn open() -> anyhow::Result<Device> {
  let device_info = nusb::list_devices()?
    .find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
    .context("device not connected")?;

  let device = device_info.open().context("failed to open device")?;
  let interface = device.claim_interface(0)?;

  Ok(Device::from_interface(interface))
}
```

### Tune

Identical interface to [libhackrf](https://github.com/greatscottgadgets/hackrf/tree/master/host/libhackrf)

```rust
device.set_freq(2412, 0).await?;
device.set_sample_rate(200_000_000, 10).await?;
device.set_baseband_filter_bandwidth(15_000_000).await?;
device.set_lna_gain(40).await?;
device.set_vga_gain(8).await?;
device.set_transceiver_mode(TransceiverMode::Receive).await?;
```

### Receive

`Device` implements `futures::Stream`, so just do what you normally do with async streams

```rust
// Returns Option<Result<Vec<Sample>>>
// Just like ordinary async stream
let chunk = device.next().await;
// Do something with the chunk received
```

### Transmit

`Device` implements `futures::Sink` for transmitting by pushing chunk of samples into Sink.
Note that you must have precise control over the sample chunk feed to avoid gaps.

```rust
device.set_transceiver_mode(TransceiverMode::Transmit).await?;

// Cast Device to `&mut impl Sink<...> + Unpin`
// to keep type across `SinkExt` calls
let sink = device.as_sink();

// Feed chunks
for chunk in samples_iter {
  sink.feed(chunk).await?;
}

// Flush when you finish transmitting
sink.flush().await?;
```

## License

Licensed under either of

* Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.
