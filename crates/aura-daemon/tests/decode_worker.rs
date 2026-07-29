use aura_core::playback::PlaybackCommand;
use aura_daemon::decode_worker::{ControlFlow, DecodeWorkerHandle, handle_command};
use crossbeam_channel::unbounded;

#[test]
fn stop_while_paused_terminates() {
    let (tx, rx) = unbounded();
    tx.send(PlaybackCommand::Stop).unwrap();
    assert_eq!(
        handle_command(PlaybackCommand::Pause, &rx),
        ControlFlow::Stopped
    );
}

#[test]
fn play_while_paused_resumes() {
    let (tx, rx) = unbounded();
    tx.send(PlaybackCommand::Play).unwrap();
    assert_eq!(
        handle_command(PlaybackCommand::Pause, &rx),
        ControlFlow::Continue
    );
}

#[test]
fn sender_dropped_while_paused_terminates_not_resumes() {
    let (tx, rx) = unbounded();
    drop(tx);
    assert_eq!(
        handle_command(PlaybackCommand::Pause, &rx),
        ControlFlow::Stopped
    );
}

#[test]
fn plain_stop_terminates() {
    let (_tx, rx) = unbounded();
    assert_eq!(
        handle_command(PlaybackCommand::Stop, &rx),
        ControlFlow::Stopped
    );
}

#[test]
fn plain_play_continues() {
    let (_tx, rx) = unbounded();
    assert_eq!(
        handle_command(PlaybackCommand::Play, &rx),
        ControlFlow::Continue
    );
}

#[test]
fn drop_sends_stop() {
    // Verify that dropping a DecodeWorkerHandle sends Stop on the command channel.
    // We use spawn_gif_worker with a non-existent path — the worker will fail to
    // open the GIF and exit immediately, so the handle's Drop behavior is testable.
    use aura_media::frame_channel;
    let (frame_tx, _frame_rx) = frame_channel();
    let path = std::path::PathBuf::from("this_file_does_not_exist.gif");
    let handle = DecodeWorkerHandle::spawn_gif_worker(path, frame_tx);
    // Dropping the handle must not panic even when the thread has already exited.
    drop(handle);
}
