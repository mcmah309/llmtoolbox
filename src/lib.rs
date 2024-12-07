mod errors;
mod tool;
mod toolbox;
mod utils;

pub use errors::*;
pub use tool::*;
pub use toolbox::*;
pub use llmtool::*;

pub fn clean_up_schema(schema: &mut serde_json::Value) {
    match schema {
        serde_json::Value::Object(map) => {
            map.remove("$schema");
            map.remove("title");
            for (_, value) in map {
                clean_up_schema_rest(value);
            }
        },
        _ => {}
    }
}

pub fn clean_up_schema_rest(schema: &mut serde_json::Value) {
    match schema {
        serde_json::Value::Object(map) => {
            map.remove("title");
            for (_, value) in map {
                clean_up_schema_rest(value);
            }
        },
        _ => {}
    }
}