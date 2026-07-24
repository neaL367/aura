use aura_storage::scanner::chrono_iso8601_now;

#[test]
fn chrono_iso8601_now_format() {
    let s = chrono_iso8601_now();
    assert!(
        chrono::DateTime::parse_from_rfc3339(&s).is_ok(),
        "expected valid RFC3339 timestamp, got: {s}"
    );
    assert!(
        s.ends_with('Z'),
        "expected UTC timestamp ending in Z, got: {s}"
    );
}

#[test]
fn chrono_iso8601_now_increasing() {
    let a = chrono_iso8601_now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let b = chrono_iso8601_now();
    assert!(
        a <= b,
        "ISO 8601 strings must sort lexicographically in chronological order: {a} <= {b}"
    );
}
