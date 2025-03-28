#[macro_export]
macro_rules! example {
    (async fn main(mut $device:ident: $device_ty:ty, $ex:ident: &$ex_ty:ty) { $($tt:tt)* }) => {
	    async fn run(mut $device: $device_ty, $ex: &$ex_ty) -> ::anyhow::Result<()> {
		    $($tt)*
	    }

	    fn main() -> ::anyhow::Result<()> {
		    let device_info = nusb::list_devices()?
				.find(|dev| dev.vendor_id() == VENDOR_ID && dev.product_id() == 0x6089)
				.context("device not connected")?;

			let device = device_info.open().context("failed to open device")?;
			let interface = device.claim_interface(0)?;

			let device = Device::from_interface(interface);

		    let ex = Executor::new();

		    ::futures::executor::block_on(ex.run(run(device, &ex)))
	    }
    };
}
