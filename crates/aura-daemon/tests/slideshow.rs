use std::collections::HashMap;

use aura_core::monitor::MonitorId;
use aura_core::slideshow_state::SlideshowState;
use aura_core::wallpaper::{WallpaperId, WallpaperMeta};
use aura_daemon::daemon::select_slideshow_items;
use aura_daemon::render_thread::RenderCommand;
use uuid::Uuid;

fn monitor(id: u8) -> MonitorId {
    MonitorId::from_uuid(Uuid::new_v5(&Uuid::NAMESPACE_DNS, &[id]))
}

fn wallpaper(path: &str) -> WallpaperMeta {
    WallpaperMeta {
        id: WallpaperId::from_path(std::path::Path::new(path)),
        path: path.into(),
        kind: aura_core::wallpaper::MediaKind::Image,
        width: 1920,
        height: 1080,
        duration_ms: 0,
        file_size: 0,
        scanned_at: String::new(),
    }
}

fn txs_for(monitors: &[MonitorId]) -> HashMap<MonitorId, crossbeam_channel::Sender<RenderCommand>> {
    monitors
        .iter()
        .map(|m| (*m, crossbeam_channel::unbounded().0))
        .collect()
}

#[test]
fn selection_only_targets_unassigned_monitors() {
    let assigned = monitor(1);
    let free = monitor(2);
    let txs = txs_for(&[assigned, free]);
    let items = vec![wallpaper("a.png"), wallpaper("b.png"), wallpaper("c.png")];

    let mut state: Option<SlideshowState> = None;
    let selected = select_slideshow_items(&txs, |m| *m == assigned, &items, &mut state);

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].0, free);
    assert!(items.iter().any(|i| i.id == selected[0].1));
}

#[test]
fn selection_advances_queue_and_avoids_repeats() {
    let free = monitor(2);
    let txs = txs_for(&[free]);
    let items = vec![wallpaper("a.png"), wallpaper("b.png"), wallpaper("c.png")];

    let mut state: Option<SlideshowState> = None;
    let first = select_slideshow_items(&txs, |_| false, &items, &mut state);
    let second = select_slideshow_items(&txs, |_| false, &items, &mut state);

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    // Consecutive flips on the same monitor must not repeat the wallpaper.
    assert_ne!(first[0].1, second[0].1);
}

#[test]
fn selection_is_empty_without_items_or_monitors() {
    let free = monitor(2);
    let txs = txs_for(&[free]);
    let mut state: Option<SlideshowState> = None;

    assert!(select_slideshow_items(&txs, |_| false, &[], &mut state).is_empty());
    assert!(
        select_slideshow_items(
            &HashMap::new(),
            |_| false,
            &[wallpaper("a.png")],
            &mut state
        )
        .is_empty()
    );
}
