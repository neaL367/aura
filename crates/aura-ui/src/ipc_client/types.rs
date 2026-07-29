use aura_ipc::protocol::DaemonStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(DaemonStatus),
    Error(String),
}
