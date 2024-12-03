use std::{any::Any, error::Error, fmt::Display};

use error_set::CoerceResult;
use serde_json::Map;

pub use llmtool::*;
pub use serde_json::{from_value, json, Value};

struct ToolBox {
    _tools: Vec<Box<dyn Tool<Box<dyn Any>>>>,
    _schema: Value,
}

impl ToolBox {
    pub fn new() -> Self {
        Self {
            _tools: Vec::new(),
            _schema: Value::Null,
        }
    }

    // todo add merge to allow merging toolboxes across crates

    pub fn add_tool(
        &mut self,
        tool: Box<dyn Tool<Box<dyn Any>>>,
    ) -> Result<(), Box<dyn Tool<Box<dyn Any>>>> {
        if self._tools.iter().any(|e| e.as_ref().name() == tool.name()) {
            return Err(tool);
        }
        self._tools.push(tool);
        // todo update schema
        Ok(())
    }

    fn call(&self, tool_call: ToolCall) -> ToolCallResult {
        for tool in &self._tools {
            if tool.name() == tool_call.name {
                return match tool.run(tool_call.args) {
                    Ok(okay) => ToolCallResult::Ok(okay),
                    Err(error) => ToolCallResult::Err(error),
                };
            }
        }
        ToolCallResult::ToolNotFound
    }

    pub fn schema(&self) -> &Value {
        &self._schema
    }

    pub fn call_from_str(&self, tool_call: &str) -> Result<ToolCallResult, StrToToolCallParseError> {
        let tool_call = self.parse_str_tool_call(tool_call)?;
        Ok(self.call(tool_call))
    }

    pub fn call_from_value(&self, tool_call: Value) -> Result<ToolCallResult, ValueToToolCallParseError> {
        let tool_call = self.parse_value_tool_call(tool_call)?;
        Ok(self.call(tool_call))
    }

    /// Parses the input string.
    pub fn parse_str_tool_call(&self, input: &str) -> Result<ToolCall, StrToToolCallParseError> {
        let value = serde_json::from_str::<Value>(input)?;
        self.parse_value_tool_call(value).coerce()
    }

    pub fn parse_value_tool_call(&self, input: Value) -> Result<ToolCall, ValueToToolCallParseError2> {
        match input {
            Value::Object(mut map) => {
                let name;
                if map.contains_key("name") {
                    name = Some(map.remove("name").unwrap());
                } else {
                    name = None;
                }
                let args;
                if map.contains_key("args") {
                    args = Some(map.remove("args").unwrap());
                } else {
                    args = None;
                };
                let (name, args) = match (name, args) {
                    (None, None) => return Err(ValueToToolCallParseError2 {has_name: false, has_args: false, ..Default::default()}),
                    (None, Some(_)) => return Err(ValueToToolCallParseError2 {has_name: false, ..Default::default() }),
                    (Some(_), None) => return Err(ValueToToolCallParseError2 { has_args: false, ..Default::default() }),
                    (Some(name), Some(args)) => (name, args),
                }
                let name = match name {
                    Value::String(name) => name,
                    _ => return Err(ValueToToolCallParseError2 { is_name_string: false, ..Default::default()}),
                };
                let args = match args {
                    Value::Object(args) => args,
                    _ => return Err(ValueToToolCallParseError2 { is_args_json: false, ..Default::default() }),
                };
                return Ok(ToolCall {
                    name: name,
                    args: args,
                });
            }
            _ => return Err(ValueToToolCallParseError2 { is_input_valid: false, ..Default::default() }),
        };
    }
}

enum ToolCallResult {
    ToolNotFound,
    Ok(Box<dyn Any>),
    Err(Box<dyn Error>),
}

pub trait Tool<T>: Send + Sync {
    /// Returns the name of the tool.
    fn name(&self) -> &'static str;

    /// Provides a description of what the tool does and when to use it.
    fn description(&self) -> &'static str;

    /// Returns the parameters for OpenAI-like function call.
    fn parameters(&self) -> &'static str;

    /// Executes the core functionality of the tool.
    fn run(&self, args: Map<String, Value>) -> Result<T, Box<dyn Error>>; //todo make async
}

struct ToolCall {
    name: String,
    args: Map<String, Value>,
}

error_set::error_set! {

    /// Error parsing a [`str`] into a [`ToolCall`]
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    StrToToolCallParseError = {
        #[display("The tool call is not valid json")]
        CouldNotConvert(serde_json::Error)
    } || ValueToToolCallParseError;

    /// Error parsing [`Value`] into a [`ToolCall`]. 
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    ValueToToolCallParseError = {
        #[display("The tool call is missing the 'name' param")]
        MissingName {
            args: Value,
        },
        #[display("The tool call is missing the 'args' param")]
        MissingArgs {
            name: String,
            map: Map<String, Value>
        },
        #[display("The tool call is missing both the 'name' and 'args' params")]
        MissingNameAndArgs {
            map: Map<String, Value>
        },
        #[display("The extracted tool call 'name' param is not a string")]
        NameNotAString {
            name: Value
        },
        #[display("The extracted tool call 'args' param is not a json object")]
        ArgsNotAMap {
            name: String,
            args: Value
        },
        #[display("The tool call is not a json object")]
        InputNotAJsonObject {
            input: Value
        },
    };
}


struct ValueToToolCallParseError2 {
    is_input_valid: bool,
    has_name: bool,
    is_name_string: bool,
    has_args: bool,
    is_args_json: bool,
}

impl Default for ValueToToolCallParseError2 {
    fn default() -> Self {
        Self {
            is_input_valid: true,
            has_name: true,
            is_name_string: true,
            has_args: true,
            is_args_json: true,
        }
    }
}

impl Display for ValueToToolCallParseError2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = "Failed to fully parse tool call input.".to_string();
        if !self.is_input_valid {
            out.push_str(" The input is not a json object.");
        }
        if !self.has_name {
            out.push_str(" The input is missing the 'name' param.");
        }
        if !self.is_name_string {
            out.push_str(" The extracted 'name' param is not a string.");
        }
        if !self.has_args {
            out.push_str(" The input is missing the 'args' param.");
        }
        if !self.is_args_json {
            out.push_str(" The extracted 'args' param is not a json object.");
        }
        write!(f, "{}", out)
    }
}