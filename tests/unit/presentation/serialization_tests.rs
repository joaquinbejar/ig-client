// Unit tests for serialization.rs

#[cfg(test)]
mod tests {
    use ig_client::presentation::serialization::{
        option_string_empty_as_none, string_as_bool_opt, string_as_float_opt,
    };
    use serde::{Deserialize, Serialize};

    // Test structs for string_as_float_opt
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct FloatTest {
        #[serde(with = "string_as_float_opt")]
        value: Option<f64>,
    }

    // Test structs for string_as_bool_opt
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct BoolTest {
        #[serde(with = "string_as_bool_opt")]
        value: Option<bool>,
    }

    // Test structs for option_string_empty_as_none
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct StringTest {
        #[serde(with = "option_string_empty_as_none")]
        value: Option<String>,
    }

    // Tests for string_as_float_opt
    #[test]
    fn test_string_as_float_opt_serialize() {
        // Test serializing Some(f64)
        let test = FloatTest { value: Some(42.5) };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":42.5}"#);

        // Test serializing None
        let test = FloatTest { value: None };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":null}"#);
    }

    #[test]
    fn test_string_as_float_opt_deserialize() {
        // Test deserializing from number
        let json = r#"{"value": 42.5}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: Some(42.5) });

        // Test deserializing from string
        let json = r#"{"value": "42.5"}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: Some(42.5) });

        // Test deserializing from null
        let json = r#"{"value": null}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: None });

        // Test deserializing from empty string
        let json = r#"{"value": ""}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: None });
    }

    #[test]
    fn test_string_as_float_opt_deserialize_numeric_variants() {
        // Negative, zero, and scientific-notation string values must all parse.
        let json = r#"{"value": "-456.78"}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(
            deserialized,
            FloatTest {
                value: Some(-456.78)
            }
        );

        let json = r#"{"value": "0.0"}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: Some(0.0) });

        let json = r#"{"value": "1.23e2"}"#;
        let deserialized: FloatTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, FloatTest { value: Some(123.0) });
    }

    #[test]
    fn test_string_as_float_opt_deserialize_errors() {
        // Test deserializing from invalid string
        let json = r#"{"value": "not-a-number"}"#;
        let result: Result<FloatTest, _> = serde_json::from_str(json);
        assert!(result.is_err());

        // Test deserializing from boolean
        let json = r#"{"value": true}"#;
        let result: Result<FloatTest, _> = serde_json::from_str(json);
        assert!(result.is_err());

        // Test deserializing from array
        let json = r#"{"value": [1, 2, 3]}"#;
        let result: Result<FloatTest, _> = serde_json::from_str(json);
        assert!(result.is_err());

        // Test deserializing from object
        let json = r#"{"value": {"a": 1}}"#;
        let result: Result<FloatTest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // Tests for string_as_bool_opt
    #[test]
    fn test_string_as_bool_opt_serialize() {
        // Test serializing Some(true)
        let test = BoolTest { value: Some(true) };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":"1"}"#);

        // Test serializing Some(false)
        let test = BoolTest { value: Some(false) };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":"0"}"#);

        // Test serializing None
        let test = BoolTest { value: None };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":null}"#);
    }

    #[test]
    fn test_string_as_bool_opt_deserialize() {
        // Test deserializing from "1"
        let json = r#"{"value": "1"}"#;
        let deserialized: BoolTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, BoolTest { value: Some(true) });

        // Test deserializing from "0"
        let json = r#"{"value": "0"}"#;
        let deserialized: BoolTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, BoolTest { value: Some(false) });

        // Test deserializing from null
        let json = r#"{"value": null}"#;
        let deserialized: BoolTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, BoolTest { value: None });

        // Test deserializing from empty string
        let json = r#"{"value": ""}"#;
        let deserialized: BoolTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, BoolTest { value: None });
    }

    #[test]
    fn test_string_as_bool_opt_deserialize_errors() {
        // Test deserializing from invalid string
        let json = r#"{"value": "invalid"}"#;
        let result: Result<BoolTest, _> = serde_json::from_str(json);
        assert!(result.is_err());

        // Test deserializing from number
        let json = r#"{"value": 1}"#;
        let result: Result<BoolTest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // Tests for option_string_empty_as_none
    #[test]
    fn test_option_string_empty_as_none_serialize() {
        // Test serializing Some(non-empty string)
        let test = StringTest {
            value: Some("hello".to_string()),
        };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":"hello"}"#);

        // Test serializing Some(empty string) - should be serialized as null
        let test = StringTest {
            value: Some("".to_string()),
        };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":null}"#);

        // Test serializing None
        let test = StringTest { value: None };
        let serialized = serde_json::to_string(&test).unwrap();
        assert_eq!(serialized, r#"{"value":null}"#);
    }

    #[test]
    fn test_option_string_empty_as_none_deserialize() {
        // Test deserializing from non-empty string
        let json = r#"{"value": "hello"}"#;
        let deserialized: StringTest = serde_json::from_str(json).unwrap();
        assert_eq!(
            deserialized,
            StringTest {
                value: Some("hello".to_string())
            }
        );

        // Test deserializing from empty string - should be deserialized as None
        let json = r#"{"value": ""}"#;
        let deserialized: StringTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, StringTest { value: None });

        // Test deserializing from null
        let json = r#"{"value": null}"#;
        let deserialized: StringTest = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, StringTest { value: None });
    }
}
