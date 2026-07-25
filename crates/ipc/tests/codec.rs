use tokio::io::{AsyncWriteExt, duplex};

use aura_ipc::codec::{read_message, write_message};
use aura_ipc::protocol::{IpcMessage, PROTOCOL_VERSION, Request, Response};

#[tokio::test]
async fn test_codec_roundtrip_request() {
    let (mut writer, mut reader) = duplex(4096);
    let msg = IpcMessage::new(Request::GetStatus);
    write_message(&mut writer, &msg).await.unwrap();
    let deserialized: IpcMessage<Request> = read_message(&mut reader).await.unwrap();
    assert_eq!(deserialized.version, PROTOCOL_VERSION);
    assert_eq!(deserialized.payload, msg.payload);
}

#[tokio::test]
async fn test_codec_roundtrip_response() {
    let (mut writer, mut reader) = duplex(4096);
    let msg = IpcMessage::new(Response::Ok);
    write_message(&mut writer, &msg).await.unwrap();
    let deserialized: IpcMessage<Response> = read_message(&mut reader).await.unwrap();
    assert_eq!(deserialized.version, PROTOCOL_VERSION);
    assert_eq!(deserialized.payload, msg.payload);
}

#[tokio::test]
async fn test_codec_connection_closed_on_read_length() {
    let (_, mut reader) = duplex(4096);
    let result: Result<IpcMessage<Request>, _> = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, aura_ipc::IpcError::ConnectionClosed),
        "expected ConnectionClosed, got {err}"
    );
}

#[tokio::test]
async fn test_codec_connection_closed_on_read_body() {
    let (mut writer, mut reader) = duplex(4096);
    writer.write_all(&42u32.to_le_bytes()).await.unwrap();
    drop(writer);
    let result: Result<IpcMessage<Request>, _> = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, aura_ipc::IpcError::ConnectionClosed),
        "expected ConnectionClosed, got {err}"
    );
}

#[tokio::test]
async fn test_codec_message_too_large_on_read() {
    let (mut writer, mut reader) = duplex(4096);
    let oversized: u32 = 5 * 1024 * 1024;
    writer.write_all(&oversized.to_le_bytes()).await.unwrap();
    let result: Result<IpcMessage<Request>, _> = read_message(&mut reader).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, aura_ipc::IpcError::MessageTooLarge { .. }),
        "expected MessageTooLarge, got {err}"
    );
}
