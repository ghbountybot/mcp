use std::path::PathBuf;
use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    // Tell cargo to rerun this if the schema changes
    println!("cargo:rerun-if-changed=schema.json");

    let schema_path = PathBuf::from("schema.json");
    let schema_str = std::fs::read_to_string(&schema_path).expect("Failed to read schema");
    let schema = serde_json::from_str::<schemars::schema::RootSchema>(&schema_str)
        .expect("Failed to parse schema");

    let settings = TypeSpaceSettings::default();
    let mut type_space = TypeSpace::new(&settings);
    type_space
        .add_root_schema(schema)
        .expect("Failed to add schema");

    // Generate the code
    let generated_code = prettyplease::unparse(&syn::parse2::<syn::File>(type_space.to_stream()).unwrap());
    
    // Add a prelude to the generated code to handle the Result type conflict
    let prelude = r#"
// This prelude ensures we don't have conflicts with the standard library Result type
use std::result::Result as StdResult;
use std::prelude::rust_2021::*;

// Rename the schema Result type to SchemaResult to avoid conflicts
"#;
    
    // Post-process the generated code to rename the Result type to SchemaResult
    let processed_code = prelude.to_string() + &generated_code
        .replace("pub struct Result {", "pub struct SchemaResult {")
        .replace(" Result {", " SchemaResult {")
        .replace("-> Result<", "-> StdResult<")
        .replace(": Result<", ": StdResult<")
        .replace("pub result: Result,", "pub result: SchemaResult,")
        .replace("Result(Result)", "Result(SchemaResult)")
        .replace("impl From<&Result>", "impl From<&SchemaResult>")
        .replace("fn from(value: &Result)", "fn from(value: &SchemaResult)")
        .replace("impl From<Result>", "impl From<SchemaResult>")
        .replace("fn from(value: Result)", "fn from(value: SchemaResult)")
        .replace("type Target = Result;", "type Target = SchemaResult;")
        .replace("fn deref(&self) -> &Result {", "fn deref(&self) -> &SchemaResult {")
        .replace("pub struct EmptyResult(pub Result);", "pub struct EmptyResult(pub SchemaResult);")
        .replace("impl From<r>", "impl From<SchemaResult>");

    // Fix the irrefutable if-let patterns - we need to be more careful with these
    let processed_code = processed_code
        .replace(
            "        if let Ok(v) = value.parse() {\n            Ok(Self::String(v))\n        } else if let Ok(v) = value.parse() {\n            Ok(Self::Integer(v))\n        } else {\n            Err(\"string conversion failed for all variants\")\n        }",
            "        if let Ok(v) = value.parse::<String>() {\n            Ok(Self::String(v))\n        } else if let Ok(v) = value.parse::<i64>() {\n            Ok(Self::Integer(v))\n        } else {\n            Err(\"string conversion failed for all variants\")\n        }"
        )
        .replace(
            "        if let Ok(v) = value.parse() {\n            Ok(Self::String(v))\n        } else if let Ok(v) = value.parse() {\n            Ok(Self::Integer(v))\n        } else {\n            Err(\"string conversion failed for all variants\")\n        }",
            "        if let Ok(v) = value.parse::<String>() {\n            Ok(Self::String(v))\n        } else if let Ok(v) = value.parse::<i64>() {\n            Ok(Self::Integer(v))\n        } else {\n            Err(\"string conversion failed for all variants\")\n        }"
        );

    let output_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("types.rs");
    std::fs::write(&output_path, processed_code)
        .expect("Failed to write generated code");
}
