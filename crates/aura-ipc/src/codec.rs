use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::IpcError;

/// Maximum permitted message size (4 MiB).
const MAX_MSG_BYTES: usize = 4 * 1024 * 1024;

/// Write a length-prefixed JSON message to `writer`.
///
/// Frame format: `[u32 LE length][JSON bytes]`
pub async fn write_message<W, T>(writer: &mut W, value: &T) -> Result<(), IpcError>
where
    W: AsyncWriteExt + Unpin,
    T: serde::Serialize,
{
    let json = serde_json::to_vec(value)?;
    if json.len() > MAX_MSG_BYTES {
        return Err(IpcError::MessageTooLarge {
            size: json.len(),
            max: MAX_MSG_BYTES,
        });
    }
    let len = json.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&json).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-prefixed JSON message from `reader`.
pub async fn read_message<R, T>(reader: &mut R) -> Result<T, IpcError>
where
    R: AsyncReadExt + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            IpcError::ConnectionClosed
        } else {
            IpcError::Io(e)
        }
    })?;

    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MSG_BYTES {
        return Err(IpcError::MessageTooLarge {
            size: len,
            max: MAX_MSG_BYTES,
        });
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            IpcError::ConnectionClosed
        } else {
            IpcError::Io(e)
        }
    })?;

    Ok(serde_json::from_slice(&buf)?)
}
