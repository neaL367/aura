use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("unsupported media type: {0}")]
    UnsupportedType(String),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("end of stream")]
    EndOfStream,

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
}
