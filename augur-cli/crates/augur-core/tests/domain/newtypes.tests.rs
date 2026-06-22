use augur_domain::domain::newtypes::{Count, NumericNewtype, Temperature, TokenCount};

#[test]
fn token_count_arithmetic() {
    let a = TokenCount::new(10);
    let b = TokenCount::new(5);
    assert_eq!((a + b).inner(), 15);
    assert_eq!((a - b).inner(), 5);
}

#[test]
fn temperature_ordering() {
    let low = Temperature::new(0.0);
    let high = Temperature::new(1.0);
    assert!(high > low);
}

#[test]
fn count_zero_constant() {
    assert_eq!(Count::ZERO.inner(), 0);
}

#[test]
fn token_count_display() {
    let t = TokenCount::new(42);
    assert_eq!(t.to_string(), "42");
}
