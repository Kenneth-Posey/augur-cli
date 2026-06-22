use augur_domain::domain::Temperature;
use augur_domain::domain::effort_level::EffortLevel;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::StringNewtype;

fn temp(v: f64) -> Temperature {
    Temperature::new(v)
}

#[test]
fn zero_temperature_is_low() {
    assert_eq!(EffortLevel::from_temperature(temp(0.0)), EffortLevel::Low);
}

#[test]
fn mid_temperature_is_medium() {
    assert_eq!(
        EffortLevel::from_temperature(temp(0.5)),
        EffortLevel::Medium
    );
}

#[test]
fn high_temperature_is_high() {
    assert_eq!(EffortLevel::from_temperature(temp(1.0)), EffortLevel::High);
}

#[test]
fn labels_match_variants() {
    assert_eq!(EffortLevel::Low.label().as_str(), "low");
    assert_eq!(EffortLevel::Medium.label().as_str(), "medium");
    assert_eq!(EffortLevel::High.label().as_str(), "high");
}
