use serde_json::{Map, Value};

use crate::FunctionCallError;

/// Tools in a struct/enum
// #[async_trait::async_trait]
pub trait Tool {
    type Output;
    type Error;

    fn function_names(&self) -> &[&'static str];

    /// The schema for functions available to call for this tool
    fn schema(&self) -> &'static Map<String, Value>;

    /// Runs the tool. This can never be called directly.
    fn call_function<'life0, 'life1, 'async_trait>(
        &'life0 self,
        name: &'life1 str,
        parameters: Map<String, Value>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Result<Self::Output, Self::Error>, FunctionCallError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait;

    // async fn call_function(
    //     &self,
    //     name: &str,
    //     parameters: Map<String, Value>,
    // ) -> Result<Result<T, E>, FunctionCallError>;
}


impl<O, E> Tool for Box<dyn Tool<Output = O, Error = E>> {
    type Output = O;

    type Error = E;

    fn function_names(&self) -> &[&'static str] {
        self.as_ref().function_names()
    }

    fn schema(&self) -> &'static Map<String, Value> {
        self.as_ref().schema()
    }

    fn call_function<'life0, 'life1, 'async_trait>(
        &'life0 self,
        name: &'life1 str,
        parameters: Map<String, Value>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Result<Self::Output, Self::Error>, FunctionCallError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait {
        self.as_ref().call_function(name, parameters)
    }
}