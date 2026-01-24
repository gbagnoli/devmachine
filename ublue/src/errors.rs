use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("An IO error occurred: {0}")]
    Io(#[from] std::io::Error),
    #[error("A formatting error occurred: {0}")]
    Fmt(#[from] std::fmt::Error),
    #[error("A custom error occurred: {0}")]
    Custom(String),
}
