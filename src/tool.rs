use serde_json::{Map, Value};

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

error_set::error_set!{

    /// An error related to dynamically calling a function, not runing the function.
    /// Either there was an error parsing the arguments or the function did not exist.
    CallError = {
        #[display("The function with name `{function_name}` was not found in the toolbox")]
        FunctionNotFound {
            function_name: String,
        },
        /// Issue related to parsing to json or to the desired schema shape.
        #[display("An issue occured paring against the schema:\n{issue}")]
        Parsing {
            issue: String,
        }
    };
}

impl CallError {
    pub fn function_not_found(function_name: String) -> Self {
        Self::FunctionNotFound { function_name }
    }

    pub fn parsing(issue: String) -> Self {
        Self::Parsing { issue }
    }
}
