use crate::orchestrator::Orchestrator;
use crate::render_thread;
use crate::slideshow_preload::SlideshowPreloader;
use aura_core::slideshow_state::SlideshowState;

/// Run one slideshow cycle: advance the queue and assign the next wallpaper to
/// each slideshow monitor. Static images are served from the preload cache
/// when available (no decode hitch at the flip); otherwise a normal
/// `SetWallpaper` command is sent.
pub fn run_slideshow_cycle(
    wallpaper_txs: &std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<render_thread::RenderCommand>,
    >,
    orchestrator: &Orchestrator,
    items: &[aura_core::wallpaper::WallpaperMeta],
    state: &mut Option<SlideshowState>,
    store: &aura_storage::slideshow_store::SlideshowStore,
    preloader: &SlideshowPreloader,
) -> Vec<(
    aura_core::monitor::MonitorId,
    aura_core::wallpaper::WallpaperId,
)> {
    let selected = select_slideshow_items(
        wallpaper_txs,
        |m| orchestrator.is_monitor_assigned(m),
        items,
        state,
    );

    for (monitor, chosen) in &selected {
        let Some(meta) = items.iter().find(|item| item.id == *chosen) else {
            continue;
        };
        let Some(tx) = wallpaper_txs.get(monitor) else {
            continue;
        };

        if let Some(frame) = preloader.take(monitor, *chosen) {
            let _ = tx.send(render_thread::RenderCommand::SetWallpaperPredecoded {
                path: meta.path.clone(),
                fit_mode: None,
                frame,
            });
        } else {
            let _ = tx.send(render_thread::RenderCommand::SetWallpaper {
                path: meta.path.clone(),
                fit_mode: None,
            });
        }
    }

    if let Some(s) = state
        && let Err(e) = store.save(s)
    {
        tracing::warn!("Failed to save slideshow state: {}", e);
    }

    selected
}

/// Advance the slideshow queue and compute the next assignment for every
/// slideshow monitor. Pure selection (no I/O, no IPC): mutates `state` so the
/// real cycle and the preloader stay in lockstep.
///
/// Returns `(monitor, chosen_wallpaper)` pairs in monitor order.
pub fn select_slideshow_items(
    wallpaper_txs: &std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<render_thread::RenderCommand>,
    >,
    is_assigned: impl Fn(&aura_core::monitor::MonitorId) -> bool,
    items: &[aura_core::wallpaper::WallpaperMeta],
    state: &mut Option<SlideshowState>,
) -> Vec<(
    aura_core::monitor::MonitorId,
    aura_core::wallpaper::WallpaperId,
)> {
    use rand::seq::SliceRandom;

    if items.is_empty() {
        return Vec::new();
    }

    let s = state.get_or_insert_with(SlideshowState::new);

    // Identify monitors without manual assignments (slideshow monitors).
    let mut slideshow_monitors: Vec<aura_core::monitor::MonitorId> = wallpaper_txs
        .keys()
        .filter(|m| !is_assigned(m))
        .copied()
        .collect();
    if slideshow_monitors.is_empty() {
        return Vec::new();
    }
    slideshow_monitors.sort_by_key(|m| m.as_uuid());

    // Sanitize queue: remove IDs no longer in the library.
    s.queue.retain(|id| items.iter().any(|item| item.id == *id));
    s.queue.shrink_to_fit();
    if s.index > s.queue.len() {
        s.index = s.queue.len().saturating_sub(1);
    }

    // If queue is too short or empty, rebuild from all library IDs.
    if s.queue.len() < slideshow_monitors.len() {
        s.queue = items.iter().map(|item| item.id).collect();
        s.queue.shuffle(&mut rand::rng());
        s.queue.shrink_to_fit();
        s.index = 0;
    }

    if s.queue.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::with_capacity(slideshow_monitors.len());
    let mut assigned_this_cycle = Vec::with_capacity(slideshow_monitors.len());

    for monitor in &slideshow_monitors {
        // Wrap-around: reshuffle when queue is exhausted.
        if s.index >= s.queue.len() {
            s.queue.shuffle(&mut rand::rng());
            s.index = 0;
            s.last_cycle += 1;
        }

        let candidate = s.queue[s.index];

        // Check duplicates: avoid same wallpaper on same monitor consecutively
        // and same wallpaper in this cycle across monitors.
        let is_repeat = s.last_wallpapers.get(monitor) == Some(&candidate)
            || assigned_this_cycle.contains(&candidate);

        if is_repeat {
            // Scan forward for a suitable alternative.
            let mut swap_idx = None;
            for offset in 1..s.queue.len() {
                let idx = (s.index + offset) % s.queue.len();
                let alt = s.queue[idx];
                if alt != candidate
                    && s.last_wallpapers.get(monitor) != Some(&alt)
                    && !assigned_this_cycle.contains(&alt)
                {
                    swap_idx = Some(idx);
                    break;
                }
            }
            if let Some(si) = swap_idx {
                s.queue.swap(s.index, si);
            }
            // No alternative found → use candidate anyway (unavoidable repeat).
        }

        let chosen = s.queue[s.index];
        s.last_wallpapers.insert(*monitor, chosen);
        assigned_this_cycle.push(chosen);
        selected.push((*monitor, chosen));

        s.index += 1;
    }

    selected
}
