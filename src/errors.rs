error_set::error_set!{

    /// An error related to dynamically calling a function, not runing the function.
    /// Either there was an error parsing the arguments or the function did not exist.
    CallError = {
        #[display("The function with name `{function_name}` was not found in the toolbox")]
        FunctionNotFound {
            function_name: String,
        },
    } || CallParsingError;

    CallParsingError = {
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