use thiserror::Error;
#[derive(Error, Debug)]
pub enum Error {
    #[error("Any: {0}")]
    Any(#[from] anyhow::Error),
}
