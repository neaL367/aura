use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Tracks allowed client PIDs for IPC pipe connections.
pub struct ClientValidator {
    allowed_pids: Arc<Mutex<HashSet<u32>>>,
}

impl ClientValidator {
    /// Create a new validator. An empty allowlist permits all clients.
    pub fn new() -> Self {
        Self {
            allowed_pids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Add a PID to the allowlist.
    pub fn allow_pid(&self, pid: u32) {
        self.allowed_pids.lock().unwrap().insert(pid);
    }

    /// Remove a PID from the allowlist.
    pub fn deny_pid(&self, pid: u32) {
        self.allowed_pids.lock().unwrap().remove(&pid);
    }

    /// Check if a PID is allowed. Returns true if the allowlist is empty
    /// (permissive mode) or if the PID is in the allowlist.
    pub fn is_allowed(&self, pid: u32) -> bool {
        let allowed = self.allowed_pids.lock().unwrap();
        allowed.is_empty() || allowed.contains(&pid)
    }

    /// Check if the validator has any explicit entries.
    pub fn has_restrictions(&self) -> bool {
        !self.allowed_pids.lock().unwrap().is_empty()
    }
}

impl Default for ClientValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ClientValidator {
    fn clone(&self) -> Self {
        Self {
            allowed_pids: Arc::clone(&self.allowed_pids),
        }
    }
}
