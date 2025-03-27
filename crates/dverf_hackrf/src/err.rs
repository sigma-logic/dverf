use thiserror::Error;

pub type Result<T = (), E = Error> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Usb: {0}")]
	Usb(#[from] nusb::transfer::TransferError),

	#[error("Invalid response")]
	Resp,
}
