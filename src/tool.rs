use serde_json::{Map, Value};

/// Tools in a struct/enum
#[async_trait::async_trait]
pub trait Tool<T, E>
{
    fn function_names(&self) -> &[&'static str];

    /// The schema for functions available to call for this tool
    fn schema(&self) -> &'static Map<String, Value>;

    /// Runs the tool. This can never be called directly. Only called by `ToolBox`.
    async fn call(
        &self,
        name: &str,
        parameters: Map<String, Value>
    ) -> Result<Result<T, E>, CallError>;
}

#[derive(Debug)]
pub struct CallError {
    reason: String,
}

impl CallError {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::error::Error for CallError {}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CallError: {}", self.reason)
    }
}