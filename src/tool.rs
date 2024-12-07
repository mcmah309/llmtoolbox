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

    /// Runs the tool. This can never be called directly. Only called by `ToolBox`.
    async fn run(
        &self,
        name: String,
        mut parameters: Map<String, Value>,
        execution_key: &ToolExecutionKey,
    ) -> Result<T, E>;
}

pub(crate) const TOOL_EXECUTION_KEY: ToolExecutionKey = ToolExecutionKey { key: 0 };

/// Prevents `Tool::run` from being called from outside a `ToolBox`. Since trait methods are always public.
pub struct ToolExecutionKey {
    #[allow(dead_code)]
    key: u8,
}
