use aura_security::ClientValidator;

#[test]
fn test_client_validator_permissive_default() {
    let validator = ClientValidator::new();
    assert!(!validator.has_restrictions());
    assert!(validator.is_allowed(1234));
}

#[test]
fn test_client_validator_explicit_allow_deny() {
    let validator = ClientValidator::new();
    validator.allow_pid(100);
    validator.allow_pid(200);

    assert!(validator.has_restrictions());
    assert!(validator.is_allowed(100));
    assert!(validator.is_allowed(200));
    assert!(!validator.is_allowed(300));

    validator.deny_pid(100);
    assert!(!validator.is_allowed(100));
    assert!(validator.is_allowed(200));
}
