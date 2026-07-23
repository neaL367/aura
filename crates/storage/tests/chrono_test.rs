use aura_storage::scanner::chrono_iso8601_now;

#[test]
fn chrono_iso8601_now_format() {
    let s = chrono_iso8601_now();
    assert!(s.starts_with("UNIX-"), "got: {s}");
    let num_part = s.trim_start_matches("UNIX-");
    assert!(!num_part.is_empty(), "no digits after UNIX- prefix");
    assert!(
        num_part.chars().all(|c| c.is_ascii_digit()),
        "expected digits, got: {num_part}",
    );
}

#[test]
fn chrono_iso8601_now_increasing() {
    let a = chrono_iso8601_now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let b = chrono_iso8601_now();
    let a_secs: u64 = a.trim_start_matches("UNIX-").parse().unwrap();
    let b_secs: u64 = b.trim_start_matches("UNIX-").parse().unwrap();
    assert!(b_secs >= a_secs, "expected b({b_secs}) >= a({a_secs})");
}
