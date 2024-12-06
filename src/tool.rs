use std::collections::HashMap;

use jsonschema::Validator;
use serde_json::{Map, Value};

/// Tools in a struct/enum
#[async_trait::async_trait]
pub trait Tool<T, E>: 'static
where
    T: 'static,
    E: 'static,
{
    fn function_names(&self) -> &[&'static str];

    /// Returns the name of the function and the call validator.
    fn function_name_to_validator(&self) -> HashMap<&'static str, Validator>;

    /// The schema for functions available to call for this tool
    fn schema(&self) -> &'static Map<String, Value>;

    /// This should never be called directly! Only called by `ToolBox`
    /// Executes the core functionality of the tool.
    async fn run(&self, name: &str, parameters: &Map<String, Value>) -> Result<T, E>;
}