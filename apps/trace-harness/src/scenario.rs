//! The scenario file that goes with a trace: the dungeon and floor ids the trace does not
//! record (`+0x63C` and `+0x63D` are not captured). A scenario is a JSON object such as
//! `{"dungeonId": 3, "floorId": 1}`. Both ids are integers from 0 to 255.

use std::fmt;

use serde_json::Value;

/// The dungeon and floor ids of a trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scenario {
    pub dungeon_id: u8,
    pub floor_id: u8,
}

/// Why a scenario file cannot be used.
#[derive(Debug, PartialEq, Eq)]
pub enum ScenarioError {
    /// The text is not JSON.
    Json(String),
    /// The JSON is not an object.
    NotAnObject,
    /// A required field is absent.
    Missing { field: &'static str },
    /// A field is not an integer from 0 to 255. `value` is the JSON text of the field.
    Invalid { field: &'static str, value: String },
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "scenario is not valid JSON: {e}"),
            Self::NotAnObject => write!(f, "scenario must be a JSON object"),
            Self::Missing { field } => write!(f, "scenario is missing \"{field}\""),
            Self::Invalid { field, value } => {
                write!(
                    f,
                    "scenario \"{field}\" must be an integer from 0 to 255, got {value}"
                )
            }
        }
    }
}

impl std::error::Error for ScenarioError {}

/// Parses a scenario file's text. Unknown fields are ignored.
pub fn parse_scenario(text: &str) -> Result<Scenario, ScenarioError> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| ScenarioError::Json(e.to_string()))?;
    let Value::Object(object) = value else {
        return Err(ScenarioError::NotAnObject);
    };
    Ok(Scenario {
        dungeon_id: id_field(&object, "dungeonId")?,
        floor_id: id_field(&object, "floorId")?,
    })
}

fn id_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<u8, ScenarioError> {
    let value = object.get(field).ok_or(ScenarioError::Missing { field })?;
    value
        .as_u64()
        .and_then(|n| u8::try_from(n).ok())
        .ok_or_else(|| ScenarioError::Invalid {
            field,
            value: value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_ids() {
        let scenario = parse_scenario(r#"{"dungeonId": 3, "floorId": 1}"#).expect("scenario");
        assert_eq!(
            scenario,
            Scenario {
                dungeon_id: 3,
                floor_id: 1
            }
        );
    }

    #[test]
    fn accepts_the_bounds_and_ignores_extra_fields() {
        let scenario =
            parse_scenario(r#"{"dungeonId": 255, "floorId": 0, "note": "x"}"#).expect("scenario");
        assert_eq!((scenario.dungeon_id, scenario.floor_id), (255, 0));
    }

    #[test]
    fn refuses_a_missing_field() {
        assert_eq!(
            parse_scenario(r#"{"dungeonId": 3}"#),
            Err(ScenarioError::Missing { field: "floorId" })
        );
    }

    #[test]
    fn refuses_a_non_integer() {
        for text in [
            r#"{"dungeonId": 1.5, "floorId": 0}"#,
            r#"{"dungeonId": "3", "floorId": 0}"#,
            r#"{"dungeonId": -1, "floorId": 0}"#,
            r#"{"dungeonId": null, "floorId": 0}"#,
        ] {
            assert!(
                matches!(
                    parse_scenario(text),
                    Err(ScenarioError::Invalid {
                        field: "dungeonId",
                        ..
                    })
                ),
                "{text}"
            );
        }
    }

    #[test]
    fn refuses_a_value_above_255() {
        let err = parse_scenario(r#"{"dungeonId": 0, "floorId": 256}"#).unwrap_err();
        assert_eq!(
            err,
            ScenarioError::Invalid {
                field: "floorId",
                value: "256".to_owned()
            }
        );
        assert!(err.to_string().contains("from 0 to 255, got 256"), "{err}");
    }

    #[test]
    fn refuses_text_that_is_not_a_json_object() {
        assert!(matches!(
            parse_scenario("[1, 2]"),
            Err(ScenarioError::NotAnObject)
        ));
        assert!(matches!(parse_scenario("{"), Err(ScenarioError::Json(_))));
    }
}
