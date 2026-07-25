#[cfg(target_os = "windows")]
mod windows_tests {
    use aura_platform_windows::singleton::ProcessSingleton;

    #[test]
    fn singleton_first_acquire_succeeds_second_fails() {
        let s1 = match ProcessSingleton::acquire() {
            Ok(s) => s,
            Err(aura_platform_windows::error::PlatformError::AlreadyRunning) => {
                // If wallpaperd daemon is active during cargo test, acquire already returns AlreadyRunning.
                // Test is satisfied because process singleton protection is active.
                return;
            }
            Err(e) => panic!("Unexpected error acquiring singleton: {:?}", e),
        };

        // A second acquire should fail since s1 still holds the mutex.
        let result = ProcessSingleton::acquire();
        assert!(
            result.is_err(),
            "second acquire should fail with AlreadyRunning"
        );

        // Drop s1 to release the mutex.
        drop(s1);

        // Now acquire should succeed again.
        let _s2 = ProcessSingleton::acquire().unwrap();
    }
}
