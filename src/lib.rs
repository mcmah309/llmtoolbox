use std::{any::Any, collections::HashMap, error::Error, fmt::Display, ops::Deref};

use error_set::CoerceResult;
use jsonschema::Validator;
use serde_json::Map;

pub use llmtool::*;
pub use serde_json::{from_value, json, Value};

/// A toolbox is a collection of tools that can be called by name with arguments.
pub struct ToolBox<O: 'static, E: Error + 'static> {
    /// all the tools that the llm can call
    all_tools: Vec<Box<dyn Tool<O, E>>>,
    /// schema to be sent to the llm
    schema: Map<String, Value>,
    /// tool name to parameter validator
    tool_name_to_validator: HashMap<&'static str, Validator>,
}

impl<O: 'static, E: Error + 'static> ToolBox<O, E> {
    pub fn new() -> Self {
        Self {
            all_tools: Vec::new(),
            schema: Map::new(),
            tool_name_to_validator: HashMap::new(),
        }
    }

    // todo add merge to allow merging toolboxes across crates

    /// Adds the `tool` to this [`Toolbox`]. If a tool with the same name already exists, will return
    /// Err with the tool.
    pub fn add_tool<T: Tool<O, E>>(&mut self, tool: T) -> Result<(), T> {
        let tool_names_to_validators = tool.function_name_to_validator();
        for tool_name in tool_names_to_validators.keys() {
            if self.tool_name_to_validator.contains_key(*tool_name) {
                return Err(tool);
            }
        }
        self.tool_name_to_validator.extend(tool_names_to_validators);
        self.schema.extend(tool.schema().clone());
        self.all_tools.push(Box::new(tool));
        Ok(())
    }

    /// Calls the tool with the given name and parameters.
    pub fn call(&self, function_call: &FunctionCall) -> Result<O, E> {
        for tool in &self.all_tools {
            for function_name in tool.function_names() {
                if *function_name == function_call.name {
                    return match tool.run(&function_call.name, &function_call.parameters) {
                        Ok(okay) => Ok(okay),
                        Err(error) => Err(error),
                    };
                }
            }
        }
        panic!("For a `ToolCall` can only be created from a `ToolBox`, for it not to be found, it must have been \
        created by another `ToolBox`.
        ") // todo make it so another toolbox could not create the tool call. Using the type system somehow? make them static? thread local static and non-send?
    }

    pub fn call_from_str(&self, tool_call: &str) -> Result<O, StrToolCallError<E>> {
        let tool_call = self.parse_str_tool_call(tool_call)?;
        self.call(&tool_call).map_err(|e| StrToolCallError::Tool(e))
    }

    pub fn call_from_value(&self, tool_call: Value) -> Result<O, ValueToolCallError<E>> {
        let tool_call = self.parse_value_tool_call(tool_call)?;
        self.call(&tool_call)
            .map_err(|e| ValueToolCallError::Tool(e))
    }

    pub fn schema(&self) -> &Map<String, Value> {
        &self.schema
    }

    /// Parses the input string.
    pub fn parse_str_tool_call(&self, input: &str) -> Result<FunctionCall, StrToolCallParseError> {
        let value = serde_json::from_str::<Value>(input)?;
        self.parse_value_tool_call(value).coerce()
    }

    fn get_validator(&self, name: &str) -> Option<&Validator> {
        self.tool_name_to_validator.get(name)
    }

    pub fn parse_value_tool_call(
        &self,
        input: Value,
    ) -> Result<FunctionCall, ValueToolCallParseError> {
        let name = match input.get("name") {
            Some(name) => name,
            None => return Err(ValueToolCallParseError::MissingName { input: input }),
        };
        let name = match name.as_str() {
            Some(name) => name,
            None => return Err(ValueToolCallParseError::NameNotAString { input: input }),
        };
        let validator = match self.get_validator(name) {
            Some(validator) => validator,
            None => {
                let name = name.to_owned();
                return Err(ValueToolCallParseError::ToolDoesNotExist {
                    input: input,
                    name: name,
                });
            }
        };
        let parameters = input.get("parameters");
        let Some(parameters) = parameters else {
            return Err(ValueToolCallParseError::MissingParameters { input: input});
        };
        if !parameters.is_object() {
            return Err(ValueToolCallParseError::ParametersNotAObject { input: input });
        }
        if let Err(error) = validator.validate(parameters) {
            let context = format!(
                r#"
                Issue: {}

                Violation Instance: {}

                Violation Path: {}

                Schema Property Violated: {}"#,
                &error.to_string(),
                &error.instance,
                &error.instance_path,
                &error.schema_path
            );
            return Err(ValueToolCallParseError::DoesNotMatchToolSchema {
                input: input,
                issue: context,
            });
        }
        let mut map = match input {
            Value::Object(map) => map,
            _ => unreachable!(),
        };
        let name = map.remove("name").unwrap();
        let name = match name {
            Value::String(name) => name,
            _ => unreachable!(),
        };
        let parameters = map.remove("parameters").unwrap();
        let parameters = match parameters {
            Value::Object(parameters) => parameters,
            _ => unreachable!(),
        };
        return Ok(FunctionCall {
            name,
            parameters,
        });
    }
}

// dev note: keep private so it is impossible to call a tool that does not exist
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FunctionCall {
    name: String,
    parameters: Map<String, Value>,
}

/// Tools in a struct/enum
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
    fn run(&self, name: &str, parameters: &Map<String, Value>) -> Result<T, E>; //todo make async
}

//************************************************************************//

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
            input: Value,
            name: String,
        },
        #[display("The tool call does not match the schema:\n'''\n{issue}\n'''for:\n{input}")]
        DoesNotMatchToolSchema {
            input: Value,
            issue: String,
        },
    };
}
