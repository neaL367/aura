use crate::orchestrator::Orchestrator;
use crate::render_thread;

pub fn run_slideshow_cycle(
    wallpaper_txs: &std::collections::HashMap<
        aura_core::monitor::MonitorId,
        crossbeam_channel::Sender<render_thread::RenderCommand>,
    >,
    orchestrator: &Orchestrator,
    items: &[aura_core::wallpaper::WallpaperMeta],
    state: &mut Option<aura_core::slideshow_state::SlideshowState>,
    store: &aura_storage::slideshow_store::SlideshowStore,
) {
    use rand::seq::SliceRandom;

    if items.is_empty() {
        return;
    }

    let s = state.get_or_insert_with(aura_core::slideshow_state::SlideshowState::new);

    // Identify monitors without manual assignments (slideshow monitors).
    let mut slideshow_monitors: Vec<aura_core::monitor::MonitorId> = wallpaper_txs
        .keys()
        .filter(|m| !orchestrator.is_monitor_assigned(m))
        .copied()
        .collect();
    if slideshow_monitors.is_empty() {
        return;
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
        return;
    }

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

        if let (Some(meta), Some(tx)) = (
            items.iter().find(|item| item.id == chosen),
            wallpaper_txs.get(monitor),
        ) {
            let _ = tx.send(render_thread::RenderCommand::SetWallpaper {
                path: meta.path.clone(),
                fit_mode: None,
            });
        }

        s.index += 1;
    }

    if let Err(e) = store.save(s) {
        tracing::warn!("Failed to save slideshow state: {}", e);
    }
}
