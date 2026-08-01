use super::H264ParseError;

/// Minimal big-endian bit reader over an RBSP byte slice.
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    pub fn bits_left(&self) -> usize {
        self.data.len() * 8 - self.bit_pos
    }

    /// Read `n` bits (max 32) as an unsigned integer.
    pub fn u(&mut self, n: usize) -> Result<u32, H264ParseError> {
        if n > 32 || self.bits_left() < n {
            return Err(H264ParseError::Truncated);
        }
        let mut value = 0u32;
        for _ in 0..n {
            let byte = self.data[self.bit_pos / 8];
            let bit = (byte >> (7 - (self.bit_pos % 8))) & 1;
            value = (value << 1) | bit as u32;
            self.bit_pos += 1;
        }
        Ok(value)
    }

    pub fn ue(&mut self) -> Result<u32, H264ParseError> {
        let mut leading_zeros = 0;
        while self.u(1)? == 0 {
            leading_zeros += 1;
            if leading_zeros > 31 {
                return Err(H264ParseError::InvalidSyntax);
            }
        }
        if leading_zeros == 0 {
            return Ok(0);
        }
        let suffix = self.u(leading_zeros)?;
        Ok((1 << leading_zeros) - 1 + suffix)
    }

    pub fn se(&mut self) -> Result<i32, H264ParseError> {
        let code_num = self.ue()?;
        let sign = if code_num % 2 == 0 { -1 } else { 1 };
        Ok(sign * code_num.div_ceil(2) as i32)
    }
}
