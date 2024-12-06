use std::error::Error;

use serde_json::Value;

error_set::error_set! {
    #[disable(From(E))]
    StrToolCallError<E: Error> = { Tool(E), } || StrFunctionCallParseError;

    #[disable(From(E))]
    ValueFunctionCallError<E: Error> = {
        Tool(E),
    } || ValueFunctionCallParseError;

    /// Error parsing a [`str`] into a [`ToolCall`]
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    StrFunctionCallParseError = {
        #[display("The tool call is not valid json")] //todo check if adding the inner error message here would help
        ConversionError(serde_json::Error),
    } || ValueFunctionCallParseError;

    /// Error parsing [`Value`] into a [`ToolCall`].
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    ValueFunctionCallParseError = {
        #[display("The tool call is missing the 'name' param for:\n{input}")]
        MissingName {
            input: Value,
        },
        #[display("The extracted tool call 'name' param is not a string for:\n{input}")]
        NameNotAString {
            input: Value
        },
        #[display("The tool call is missing the 'parameters' param for:\n{input}")]
        MissingParameters {
            input: Value,
        },
        #[display("The extracted tool call 'parameters' param is not an object for:\n{input}")]
        ParametersNotAObject {
            input: Value
        },
        #[display("The tool with name '{name}' does not exist for:\n{input}")]
        ToolDoesNotExist {
            name: String,
            input: Value,
        },
        #[display("The tool call '{name}''s 'parameters' does not match the schema:\n'''\n{issue}\n'''for:\n{parameters_schema}")]
        ParametersSchemaMismatch {
            name: String,
            issue: String,
            parameters_schema: Value,
        },
    };
}