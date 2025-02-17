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

    let output_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("types.rs");
    std::fs::write(
        &output_path,
        prettyplease::unparse(&syn::parse2::<syn::File>(type_space.to_stream()).unwrap()),
    )
    .expect("Failed to write generated code");
}
