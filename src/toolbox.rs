use std::{collections::HashMap, error::Error};

use error_set::CoerceResult;
use jsonschema::Validator;
use serde_json::{Map, Value};

use crate::{
    errors::{
        StrToolCallError, StrToolCallParseError, ValueToolCallError, ValueToolCallParseError,
    },
    tool::Tool,
    utils::unwrap_match,
    TOOL_EXECUTION_KEY,
};

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
    /// The provided [ToolCall] must come from this [ToolBox]'s `parse_*` methods.
    /// Passing a [ToolCall] from a different [ToolBox] may cause a panic.
    pub async fn call(&self, tool_call: ToolCall) -> Result<O, E> {
        for tool in &self.all_tools {
            for function_name in tool.function_names() {
                if *function_name == tool_call.name {
                    return tool
                        .run(tool_call.name, tool_call.parameters, &TOOL_EXECUTION_KEY)
                        .await;
                }
            }
        }
        let name = tool_call.name;
        panic!("Tool named `{name}` not found in this `ToolBox`. \
        A `ToolCall` can only be created from a `ToolBox` and is only valid for `ToolBox` that created it.")
    }

    pub async fn call_from_str(&self, tool_call: &str) -> Result<O, StrToolCallError<E>> {
        let tool_call = self.parse_str_tool_call(tool_call)?;
        self.call(tool_call)
            .await
            .map_err(|e| StrToolCallError::Tool(e))
    }

    pub async fn call_from_value(&self, tool_call: Value) -> Result<O, ValueToolCallError<E>> {
        let tool_call = self.parse_value_tool_call(tool_call)?;
        self.call(tool_call)
            .await
            .map_err(|e| ValueToolCallError::Tool(e))
    }

    pub fn schema(&self) -> &Map<String, Value> {
        &self.schema
    }

    /// Parses the input string.
    pub fn parse_str_tool_call(&self, input: &str) -> Result<ToolCall, StrToolCallParseError> {
        let value = serde_json::from_str::<Value>(input)?;
        self.parse_value_tool_call(value).coerce()
    }

    fn get_validator(&self, name: &str) -> Option<&Validator> {
        self.tool_name_to_validator.get(name)
    }

    pub fn parse_value_tool_call(&self, input: Value) -> Result<ToolCall, ValueToolCallParseError> {
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
            return Err(ValueToolCallParseError::MissingParameters { input: input });
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
            let name = name.to_owned();
            let mut map = unwrap_match!(input, Value::Object);
            return Err(ValueToolCallParseError::ParametersSchemaMismatch {
                name,
                issue: context,
                parameters_schema: map.remove("parameters").unwrap(),
            });
        }
        let mut map = unwrap_match!(input, Value::Object);
        let name = map.remove("name").unwrap();
        let name = unwrap_match!(name, Value::String);
        let parameters = map.remove("parameters").unwrap();
        let parameters = unwrap_match!(parameters, Value::Object);
        return Ok(ToolCall { name, parameters });
    }
}

// dev note: keep private so it is impossible to call a tool that does not exist
/// A valid call for a tool in the [ToolBox] it came from.
/// Do not pass to a different [ToolBox] than the one that created this.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ToolCall {
    name: String,
    parameters: Map<String, Value>,
}
