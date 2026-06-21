use augur_domain::domain::effort_level::EffortLevel;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::newtypes::Temperature;
use augur_domain::domain::string_newtypes::StringNewtype;

#[test]
fn from_temperature_zero_is_low() {
    assert_eq!(
        EffortLevel::from_temperature(Temperature::new(0.0)),
        EffortLevel::Low
    );
}

#[test]
fn from_temperature_point_five_is_medium() {
    assert_eq!(
        EffortLevel::from_temperature(Temperature::new(0.5)),
        EffortLevel::Medium
    );
}

#[test]
fn from_temperature_above_point_five_is_high() {
    assert_eq!(
        EffortLevel::from_temperature(Temperature::new(0.6)),
        EffortLevel::High
    );
    assert_eq!(
        EffortLevel::from_temperature(Temperature::new(1.0)),
        EffortLevel::High
    );
}

#[test]
fn label_values_match_variant_names() {
    assert_eq!(EffortLevel::Low.label().as_str(), "low");
    assert_eq!(EffortLevel::Medium.label().as_str(), "medium");
    assert_eq!(EffortLevel::High.label().as_str(), "high");
}
