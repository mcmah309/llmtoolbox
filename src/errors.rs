use std::error::Error;

use serde_json::Value;

error_set::error_set! {
    #[disable(From(E))]
    StrToolCallError<E: Error> = { Tool(E), } || StrToolCallParseError;

    #[disable(From(E))]
    ValueToolCallError<E: Error> = {
        Tool(E),
    } || ValueToolCallParseError;

    /// Error parsing a [`str`] into a [`ToolCall`]
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    StrToolCallParseError = {
        #[display("The tool call is not valid json")] //todo check if adding the inner error message here would help
        ConversionError(serde_json::Error),
    } || ValueToolCallParseError;

    /// Error parsing [`Value`] into a [`ToolCall`].
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    ValueToolCallParseError = {
        #[display("The tool call is missing the `name` field in:\n{input}")]
        MissingName {
            input: Value,
        },
        #[display("The tool call `name` field is not a string in:\n{input}")]
        NameNotAString {
            input: Value
        },
        #[display("The tool call is missing the `parameters` field in:\n{input}")]
        MissingParameters {
            input: Value,
        },
        #[display("The tool call `parameters` field is not an object in:\n{input}")]
        ParametersNotAObject {
            input: Value
        },
        #[display("The tool with `name` field `{name}` does not exist for tool call:\n{input}")]
        ToolDoesNotExist {
            name: String,
            input: Value,
        },
        #[display("The `parameters` field in tool call for `{name}` does not match the schema. \
        The provided value was:\
        \n'''\n{parameters_schema}\n'''\n\
        The schema violation error was:\
        \n'''\n{issue}\n'''")]
        ParametersSchemaMismatch {
            name: String,
            issue: String,
            parameters_schema: Value,
        },
    };
}
