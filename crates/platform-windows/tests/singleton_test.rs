#[cfg(target_os = "windows")]
mod windows_tests {
    use aura_platform_windows::singleton::ProcessSingleton;

    #[test]
    fn singleton_first_acquire_succeeds_second_fails() {
        // Acquire the global mutex for the first time.
        let s1 = ProcessSingleton::acquire().unwrap();

        // A second acquire should fail since s1 still holds the mutex.
        let result = ProcessSingleton::acquire();
        assert!(result.is_err(), "second acquire should fail with AlreadyRunning");

        // Drop s1 to release the mutex.
        drop(s1);

        // Now acquire should succeed again.
        let _s2 = ProcessSingleton::acquire().unwrap();
    }
}
