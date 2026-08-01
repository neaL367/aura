use std::thread::JoinHandle;

use crate::orchestrator::Orchestrator;

use super::DaemonError;

/// Spawn the async IPC server on a dedicated Tokio thread immediately
/// at process startup so UI client connections are accepted without
/// waiting for GPU or WorkerW init. Returns the server thread and the
/// watch channel used to signal its shutdown.
pub(super) fn spawn_ipc_server(
    orchestrator: &Orchestrator,
    ready_tx: std::sync::mpsc::SyncSender<()>,
) -> Result<(JoinHandle<()>, tokio::sync::watch::Sender<bool>), DaemonError> {
    let orchestrator_ipc = orchestrator.clone();
    let (ipc_server_shutdown_tx, ipc_server_shutdown_rx) = tokio::sync::watch::channel(false);
    let ipc_thread = std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create Tokio runtime for IPC: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let handler = Box::new(move |req| orchestrator_ipc.handle_request(req));
                let server = aura_ipc::server::IpcServer::new(handler)
                    .with_client_validation()
                    .on_ready(move || {
                        let _ = ready_tx.send(());
                    });
                if let Err(e) = server.serve(ipc_server_shutdown_rx).await {
                    tracing::error!("IPC server error: {}", e);
                }
            });
        })
        .map_err(|_| DaemonError::ThreadSpawn)?;

    Ok((ipc_thread, ipc_server_shutdown_tx))
}
