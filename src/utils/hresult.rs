use windows::core::HRESULT;
use crate::utils::error::Result;

/// Helper to convert a raw COM HRESULT into a Result<(), AppError>.
#[inline]
pub fn check(hr: HRESULT) -> Result<()> {
    hr.ok().map_err(Into::into)
}
