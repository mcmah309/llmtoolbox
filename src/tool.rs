use serde_json::{Map, Value};

use crate::CallError;

/// Tools in a struct/enum
// #[async_trait::async_trait]
pub trait Tool<T, E> {
    fn function_names(&self) -> &[&'static str];

    /// The schema for functions available to call for this tool
    fn schema(&self) -> &'static Map<String, Value>;

    /// Runs the tool. This can never be called directly. Only called by `ToolBox`.
    fn call<'life0, 'life1, 'async_trait>(
        &'life0 self,
        name: &'life1 str,
        parameters: Map<String, Value>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Result<T, E>, CallError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait;

    // async fn call(
    //     &self,
    //     name: &str,
    //     parameters: Map<String, Value>,
    // ) -> Result<Result<T, E>, CallError>;
}
