use thiserror::Error;

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
