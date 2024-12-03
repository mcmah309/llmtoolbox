use std::{any::Any, collections::HashMap, error::Error, fmt::Display};

use error_set::CoerceResult;
use jsonschema::Validator;
use serde_json::Map;

pub use llmtool::*;
pub use serde_json::{from_value, json, Value};

/// A toolbox is a collection of tools that can be called by name with arguments.
pub struct ToolBox<R> {
    _tools: Vec<Box<dyn Tool<R>>>,
    _schema: Map<String, Value>,
    _validators: HashMap<String, Validator>,
}

impl<R> ToolBox<R> {
    pub fn new() -> Self {
        Self {
            _tools: Vec::new(),
            _schema: Map::new(),
            _validators: HashMap::new(),
        }
    }

    // todo add merge to allow merging toolboxes across crates

    /// Adds the `tool` to this [`Toolbox`]. If a tool with the same name already exists, will return
    /// Err with the tool.
    pub fn add_tool<T: Tool<R> + 'static>(
        &mut self,
        tool: T,
    ) -> Result<(), T> {
        if self._tools.iter().any(|e| e.as_ref().name() == tool.name()) {
            return Err(tool);
        }
        let validator = Validator::new(&Value::Object(tool.schema().clone()))
            .expect("The macro should not be able to create an invalid schema"); //todo remove
        self._validators.insert(tool.name().to_owned(), validator);
        self._schema.extend(tool.schema().clone());
        self._tools.push(Box::new(tool));
        Ok(())
    }

    /// Calls the tool with the given name and args.
    pub fn call(&self, tool_call: ToolCallArgs) -> Result<R, ToolCallError> {
        for tool in &self._tools {
            if tool.name() == tool_call.name {
                return match tool.run(tool_call.args) {
                    Ok(okay) => Ok(okay),
                    Err(error) => Err(ToolCallError::Tool(error)),
                };
            }
        }
        Err(ToolCallError::ToolNotFound(ToolNotFoundError {
            tool_call: tool_call,
        }))
    }

    pub fn call_from_str(&self, tool_call: &str) -> Result<R, StrToolCallError> {
        let tool_call = self.parse_str_tool_call(tool_call)?;
        self.call(tool_call).coerce()
    }

    pub fn call_from_value(&self, tool_call: Value) -> Result<R, ValueToolCallError> {
        let tool_call = self.parse_value_tool_call(tool_call)?;
        self.call(tool_call).coerce()
    }

    pub fn schema(&self) -> &Map<String, Value> {
        &self._schema
    }

    /// Parses the input string.
    pub fn parse_str_tool_call(
        &self,
        input: &str,
    ) -> Result<ToolCallArgs, StrToToolCallParseError> {
        let value = serde_json::from_str::<Value>(input)?;
        self.parse_value_tool_call(value).coerce()
    }

    fn get_validator(&self, name: &str) -> Option<&Validator> {
        self._validators.get(name)
    }

    pub fn parse_value_tool_call(
        &self,
        input: Value,
    ) -> Result<ToolCallArgs, ValueToToolCallParseError> {
        let name = match input.get("name") {
            Some(name) => name,
            None => return Err(ValueToToolCallParseError::MissingName { input: input }),
        };
        let name = match name.as_str() {
            Some(name) => name,
            None => return Err(ValueToToolCallParseError::NameNotAString { input: input }),
        };
        let validator = match self.get_validator(name) {
            Some(validator) => validator,
            None => {
                let name = name.to_owned();
                return Err(ValueToToolCallParseError::ToolDoesNotExist {
                    input: input,
                    name: name,
                });
            }
        };
        if let Err(error) = validator.validate(&input) {
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
            return Err(ValueToToolCallParseError::DoesNotMatchToolSchema {
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
        let args = map.remove("args").unwrap();
        let args = match args {
            Value::Object(args) => args,
            _ => unreachable!(),
        };
        return Ok(ToolCallArgs {
            name: name,
            args: args,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ToolCallArgs {
    name: String,
    args: Map<String, Value>,
}

pub trait Tool<T> {
    /// Returns the name of the tool.
    fn name(&self) -> &'static str;

    fn schema(&self) -> &'static Map<String, Value>;

    fn validator(&self) -> &'static Validator;

    /// Executes the core functionality of the tool.
    fn run(&self, args: Map<String, Value>) -> Result<T, Box<dyn Error>>; //todo make async
}

//************************************************************************//

error_set::error_set! {

    /// Error parsing a [`str`] into a [`ToolCall`]
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    StrToToolCallParseError = {
        #[display("The tool call is not valid json")] //todo check if adding the error message here would help
        ConversionError(serde_json::Error),
        ValueToToolCallParseError(ValueToToolCallParseError)
    };

    StrToolCallError = StrToToolCallParseError || ToolCallError;

    ValueToolCallError = {
        ValueToToolCallParseError(ValueToToolCallParseError)
    } || ToolCallError;

    ToolCallError = {
        ToolNotFound(ToolNotFoundError),
        /// The tool execution failed.
        Tool(Box<dyn Error>),
    };

    /// Error parsing [`Value`] into a [`ToolCall`].
    /// The display message for this type is human/llm readable.
    /// Thus it is okay to pass this back to the llm to try again.
    ValueToToolCallParseError = {
        #[display("The tool call is missing the 'name' param")]
        MissingName {
            input: Value,
        },
        #[display("The extracted tool call 'name' param is not a string")]
        NameNotAString {
            input: Value
        },
        #[display("The tool call with name '{name}' does not exist")]
        ToolDoesNotExist {
            input: Value,
            name: String,
        },
        #[display("The tool call does not match the schema:\n'''\n{issue}\n'''")]
        DoesNotMatchToolSchema {
            input: Value,
            issue: String,
        },
    };
}

#[derive(Debug)]
pub struct ToolNotFoundError {
    tool_call: ToolCallArgs,
}

impl Error for ToolNotFoundError {}

impl Display for ToolNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tool not found with name `{}`", self.tool_call.name)
    }
}
