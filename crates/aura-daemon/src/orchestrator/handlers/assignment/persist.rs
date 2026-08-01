use aura_core::monitor::MonitorId;
use aura_core::wallpaper::{FitMode, WallpaperId};

use super::OrchestratorState;

/// Helper function to deduplicate persisting assignment state to `aura.toml`.
pub(super) fn persist_assignment_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
    wallpaper_id: WallpaperId,
    fit_mode: Option<FitMode>,
) -> Result<(), String> {
    let effective_fit = fit_mode.unwrap_or_default();
    state
        .mutate_config(|config| {
            if let Some(pos) = config
                .assignments
                .iter()
                .position(|a| a.monitor_id == monitor_id)
            {
                config.assignments[pos].wallpaper_id = wallpaper_id;
                if fit_mode.is_some() {
                    config.assignments[pos].fit_mode = effective_fit;
                }
            } else {
                config
                    .assignments
                    .push(aura_core::monitor::MonitorAssignment {
                        monitor_id,
                        wallpaper_id,
                        fit_mode: effective_fit,
                    });
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist wallpaper assignment: {}", e);
            format!("Failed to save assignment: {}", e)
        })?;
    Ok(())
}

/// Helper function to deduplicate persisting fit mode updates to `aura.toml`.
pub(super) fn persist_fit_mode_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
    fit_mode: FitMode,
) -> Result<(), String> {
    state
        .mutate_config(|config| {
            if let Some(pos) = config
                .assignments
                .iter()
                .position(|a| a.monitor_id == monitor_id)
            {
                config.assignments[pos].fit_mode = fit_mode;
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist fit mode: {}", e);
            format!("Failed to save fit mode: {}", e)
        })?;
    Ok(())
}

/// Helper function to deduplicate persisting assignment removals from `aura.toml`.
pub(super) fn persist_remove_assignment_config(
    state: &mut OrchestratorState,
    monitor_id: MonitorId,
) -> Result<(), String> {
    state
        .mutate_config(|config| {
            if let Some(pos) = config
                .assignments
                .iter()
                .position(|a| a.monitor_id == monitor_id)
            {
                config.assignments.remove(pos);
            }
        })
        .map_err(|e| {
            tracing::error!("Failed to persist assignment removal: {}", e);
            format!("Failed to save assignment removal: {}", e)
        })?;
    Ok(())
}
