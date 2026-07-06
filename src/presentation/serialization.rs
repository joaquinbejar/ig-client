/// Module for handling the conversion between string and optional float values
///
/// This module provides serialization and deserialization functions for converting
/// between `Option<f64>` and string representations used in the IG Markets API.
pub mod string_as_float_opt {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    /// Serializes an optional float value to its string representation
    ///
    /// # Arguments
    /// * `value` - The optional float value to serialize
    /// * `serializer` - The serializer to use
    ///
    /// # Returns
    /// A Result containing the serialized value or an error
    pub fn serialize<S>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_f64(*v), // Serialize as a number
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes a string representation to an optional float value
    ///
    /// # Arguments
    /// * `deserializer` - The deserializer to use
    ///
    /// # Returns
    /// A Result containing the deserialized optional float value or an error
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;

        match value {
            Value::Null => Ok(None),
            Value::Number(num) => {
                if let Some(float) = num.as_f64() {
                    Ok(Some(float))
                } else {
                    Err(serde::de::Error::custom("Expected a float"))
                }
            }
            Value::String(s) => {
                if s.is_empty() {
                    return Ok(None);
                }
                s.parse::<f64>().map(Some).map_err(|_| {
                    serde::de::Error::custom(format!("Failed to parse string as float: {s}"))
                })
            }
            _ => Err(serde::de::Error::custom("Expected null, number or string")),
        }
    }
}

/// Module for handling the conversion between string and optional integer values
///
/// This module provides serialization and deserialization functions for converting
/// between `Option<i64>` and string representations used in the IG Markets API.
/// It is the integer counterpart of [`string_as_float_opt`] and is intended for
/// wire values that are conceptually integers (for example epoch-millisecond
/// timestamps) where float parsing would risk silent rounding.
pub mod string_as_int_opt {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    /// Serializes an optional integer value to a JSON number
    ///
    /// # Arguments
    /// * `value` - The optional integer value to serialize
    /// * `serializer` - The serializer to use
    ///
    /// # Returns
    /// A Result containing the serialized value or an error
    pub fn serialize<S>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_i64(*v), // Serialize as a number
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes a string or number representation to an optional integer value
    ///
    /// Accepts a JSON `null` (mapped to `None`), a JSON integer, or a string
    /// holding an integer (an empty string maps to `None`). A non-integer or
    /// otherwise malformed value yields a deserialization error.
    ///
    /// # Arguments
    /// * `deserializer` - The deserializer to use
    ///
    /// # Returns
    /// A Result containing the deserialized optional integer value or an error
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;

        match value {
            Value::Null => Ok(None),
            Value::Number(num) => {
                if let Some(int) = num.as_i64() {
                    Ok(Some(int))
                } else {
                    Err(serde::de::Error::custom("Expected an integer"))
                }
            }
            Value::String(s) => {
                if s.is_empty() {
                    return Ok(None);
                }
                s.parse::<i64>().map(Some).map_err(|_| {
                    serde::de::Error::custom(format!("Failed to parse string as integer: {s}"))
                })
            }
            _ => Err(serde::de::Error::custom("Expected null, number or string")),
        }
    }
}

/// Module for handling the conversion between string and optional boolean values
///
/// This module provides serialization and deserialization functions for converting
/// between `Option<bool>` and string representations ("0" and "1") used in the IG Markets API.
pub mod string_as_bool_opt {
    use serde::{self, Deserialize, Deserializer, Serializer};

    /// Serializes an optional boolean value to its string representation
    ///
    /// Converts true to "1" and false to "0"
    ///
    /// # Arguments
    /// * `value` - The optional boolean value to serialize
    /// * `serializer` - The serializer to use
    ///
    /// # Returns
    /// A Result containing the serialized value or an error
    pub fn serialize<S>(value: &Option<bool>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => {
                let s = if *v { "1" } else { "0" };
                serializer.serialize_str(s)
            }
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes a string representation to an optional boolean value
    ///
    /// Converts "1" to true and "0" to false
    ///
    /// # Arguments
    /// * `deserializer` - The deserializer to use
    ///
    /// # Returns
    /// A Result containing the deserialized optional boolean value or an error
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                // Handle empty strings as None
                if s.is_empty() {
                    return Ok(None);
                }
                match s.as_str() {
                    "0" => Ok(Some(false)),
                    "1" => Ok(Some(true)),
                    _ => Err(serde::de::Error::custom(format!(
                        "Invalid boolean value: {s}"
                    ))),
                }
            }
            None => Ok(None),
        }
    }
}

/// Module for handling empty strings as None in `Option<String>` fields
///
/// This module provides serialization and deserialization functions for converting
/// between empty strings and None values in `Option<String>` fields.
pub mod option_string_empty_as_none {
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    /// Serializes an optional string value, treating empty strings as None
    ///
    /// # Arguments
    /// * `value` - The optional string value to serialize
    /// * `serializer` - The serializer to use
    ///
    /// # Returns
    /// A Result containing the serialized value or an error
    pub fn serialize<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(s) if s.is_empty() => serializer.serialize_none(),
            _ => value.serialize(serializer),
        }
    }

    /// Deserializes a value to an optional string, treating empty strings as None
    ///
    /// # Arguments
    /// * `deserializer` - The deserializer to use
    ///
    /// # Returns
    /// A Result containing the deserialized optional string value or an error
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) if s.is_empty() => Ok(None),
            _ => Ok(opt),
        }
    }
}
