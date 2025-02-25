use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct TestInput {
    message: String,
}

#[test]
fn test_schema_conversion() {
    // This test verifies that SchemaObject can be converted to Value

    // Get the schema for our test type
    let schema = schemars::schema_for!(TestInput);
    
    // Try to convert it to a serde_json::Value
    let value = serde_json::to_value(schema).unwrap();
    
    // Verify we got a valid JSON object
    assert!(value.is_object());
} 