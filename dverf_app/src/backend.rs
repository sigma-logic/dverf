use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
};

use arc_swap::ArcSwapOption;
use dverf::device::{BoardId, BoardRev, Device, VENDOR_ID};
use futures::future;
use futures_concurrency::future::TryJoin;
use nusb::{DeviceId, DeviceInfo, hotplug::HotplugEvent};
use smol::stream::StreamExt;
use tracing::error;

#[derive(Default)]
pub struct Backend {
	device_description: ArcSwapOption<DeviceDescription>,
	connected: AtomicBool,
}

impl Backend {
	pub async fn run(self: Arc<Self>) {
		let device_task = smol::spawn({
			let slf = Arc::clone(&self);

			async move {
				let mut hotplug = match nusb::watch_devices() {
					Ok(it) => it,
					Err(err) => {
						error!(?err, "Unable to watch for devices");
						return;
					}
				};

				while let Some(event) = hotplug.next().await {
					match event {
						HotplugEvent::Connected(dev) => {
							if dev.vendor_id() == VENDOR_ID {
								if let Some((descr, _dev)) = try_open_device(&dev).await {
									slf.device_description.store(Some(Arc::new(descr)));
									slf.connected.store(true, Ordering::Relaxed);
								}
							}
						}
						HotplugEvent::Disconnected(dev) => {
							if let Some(dev_descr) = &*slf.device_description.load() {
								if dev_descr.is(dev) {
									slf.device_description.store(None);
									slf.connected.store(false, Ordering::Relaxed);
								}
							}
						}
					}
				}
			}
		});

		future::join_all([device_task]).await;
	}
}

impl Backend {
	pub fn device_description(&self) -> Option<Arc<DeviceDescription>> {
		self.device_description.load().clone()
	}
}

pub struct DeviceDescription {
	pub id: DeviceId,
	pub info: DeviceInfo,
	pub board_id: BoardId,
	pub board_rev: BoardRev,
	pub version: String,
}

impl DeviceDescription {
	fn is(&self, id: DeviceId) -> bool {
		self.id == id
	}
}

async fn try_open_device(device_info: &DeviceInfo) -> Option<(DeviceDescription, Device)> {
	let dev = match device_info.open() {
		Ok(dev) => dev,
		Err(err) => {
			error!(?err, "Failed to open USB device");
			return None;
		}
	};

	let interface = match dev.claim_interface(0) {
		Ok(it) => it,
		Err(err) => {
			error!(?err, "Failed to claim interface 0");
			return None;
		}
	};

	let dev = Device::from_interface(interface);

	let (id, rev, version) = match (dev.board_id(), dev.board_rev(), dev.version()).try_join().await {
		Ok(it) => it,
		Err(err) => {
			error!(?err, "Board identification failed");
			return None;
		}
	};

	Some((
		DeviceDescription {
			id: device_info.id().clone(),
			info: device_info.clone(),
			board_id: id,
			board_rev: rev,
			version,
		},
		dev,
	))
}
