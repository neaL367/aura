use aura_core::playback::PlaybackCommand;
use crossbeam_channel::unbounded;
use wallpaperd::decode_worker::{ControlFlow, DecodeWorkerHandle, handle_command};

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
    let (tx, rx) = unbounded();
    {
        let handle = DecodeWorkerHandle { command_sender: tx };
        drop(handle);
    }
    assert_eq!(rx.recv().unwrap(), PlaybackCommand::Stop);
}
