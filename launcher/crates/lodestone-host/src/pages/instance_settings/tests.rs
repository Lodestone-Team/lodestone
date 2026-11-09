use launcher_core::cores::admin::instance_settings::{ConfigurableValue, ConfigurableValueType};

use super::field::{FieldKind, Problem, is_command_line, unescape_motd, value_of};

fn int(min: Option<i64>, max: Option<i64>) -> ConfigurableValueType {
    ConfigurableValueType::Integer { min, max }
}

#[test]
fn numbers_are_checked_as_the_dashboard_does() {
    let port = ConfigurableValueType::UnsignedInteger {
        min: Some(1),
        max: Some(65535),
    };
    assert_eq!(
        value_of(&port, "25565", true),
        Ok(Some(ConfigurableValue::UnsignedInteger(25565)))
    );
    assert_eq!(value_of(&port, "", true), Err(Problem::Empty));
    assert_eq!(
        value_of(&port, "", false),
        Err(Problem::NotANumber),
        "while typing, empty is only not a number"
    );
    assert_eq!(value_of(&port, "12a", true), Err(Problem::NotANumber));
    assert_eq!(value_of(&port, "1.5", true), Err(Problem::NotANumber));
    assert_eq!(value_of(&port, "0", true), Err(Problem::BelowMin("1".into())));
    assert_eq!(
        value_of(&port, "70000", true),
        Err(Problem::AboveMax("65535".into()))
    );
    let unsigned = ConfigurableValueType::UnsignedInteger {
        min: None,
        max: None,
    };
    assert_eq!(
        value_of(&unsigned, "-1", true),
        Err(Problem::BelowMin("0".into()))
    );
    assert_eq!(
        value_of(&int(None, None), "-4", true),
        Ok(Some(ConfigurableValue::Integer(-4)))
    );
}

#[test]
fn floats_and_text() {
    let float = ConfigurableValueType::Float {
        min: Some(0.),
        max: Some(1.),
    };
    assert_eq!(
        value_of(&float, "0.5", true),
        Ok(Some(ConfigurableValue::Float(0.5)))
    );
    assert_eq!(value_of(&float, "inf", true), Err(Problem::NotANumber));
    assert_eq!(value_of(&float, "NaN", true), Err(Problem::NotANumber));
    assert_eq!(value_of(&float, "2", true), Err(Problem::AboveMax("1".into())));
    let string = ConfigurableValueType::String { regex: None };
    assert_eq!(
        value_of(&string, "", true),
        Ok(Some(ConfigurableValue::String(String::new()))),
        "text may be empty"
    );
    assert_eq!(value_of(&ConfigurableValueType::Boolean, "x", true), Ok(None));
}

#[test]
fn field_kinds() {
    let string = ConfigurableValueType::String { regex: None };
    assert_eq!(FieldKind::of(&string, false), Some(FieldKind::Text));
    assert_eq!(FieldKind::of(&string, true), Some(FieldKind::Password));
    assert_eq!(FieldKind::of(&int(None, None), false), Some(FieldKind::Number));
    assert_eq!(FieldKind::of(&ConfigurableValueType::Boolean, false), None);
    assert_eq!(
        FieldKind::of(
            &ConfigurableValueType::Enum {
                options: vec!["a".into()]
            },
            false
        ),
        None
    );
}

#[test]
fn command_line_settings() {
    assert!(is_command_line("java_cmd"));
    assert!(is_command_line("cmd_args"));
    assert!(!is_command_line("motd"));
}

#[test]
fn motd_escapes() {
    assert_eq!(unescape_motd(r"A\nB"), "A\nB");
    assert_eq!(unescape_motd(r"§aGreen"), "§aGreen");
    assert_eq!(unescape_motd(r"bad \u12"), r"bad \u12");
    assert_eq!(unescape_motd(r"slash\\"), r"slash\\");
}
