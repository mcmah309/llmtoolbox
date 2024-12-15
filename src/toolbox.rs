use serde_json::{Map, Value};

use crate::{utils::unwrap_match, CallError, CallParsingError, Tool};

/// A toolbox is a collection of tools that can be called by name with arguments.
pub struct ToolBox<O, E> {
    /// all the tools that the llm can call
    all_tools: Vec<Box<dyn Tool<O, E>>>,
    /// schema to be sent to the llm
    schema: Map<String, Value>,
}

impl<O, E> ToolBox<O, E> {
    pub fn new() -> Self {
        Self {
            all_tools: Vec::new(),
            schema: Map::new(),
        }
    }

    // todo add merge to allow merging toolboxes across crates

    /// Adds the `tool` to this [`Toolbox`]. If a tool with the same name already exists, will return
    /// Err with the tool.
    pub fn add_tool<T: Tool<O, E> + 'static>(&mut self, tool: T) -> Result<(), T> {
        for existing_function_name in self.all_tools.iter().map(|e| e.function_names()).flatten() {
            for new_function_name in tool.function_names() {
                if existing_function_name == new_function_name {
                    return Err(tool);
                }
            }
        }
        self.schema.extend(tool.schema().clone());
        self.all_tools.push(Box::new(tool));
        Ok(())
    }

    /// Calls the tool with the given name and parameters.
    pub async fn call(&self, tool_call: Value) -> Result<Result<O, E>, CallError> {
        let tool_call = self.value_split_into_tool_call(tool_call)?;
        self.call_from_tool_call(tool_call).await
    }

    /// Calls the tool with the given name and parameters.
    pub async fn call_from_str(&self, tool_call: &str) -> Result<Result<O, E>, CallError> {
        let tool_call = self.str_split_into_tool_call(tool_call)?;
        self.call_from_tool_call(tool_call).await
    }

    async fn call_from_tool_call(&self, tool_call: ToolCall) -> Result<Result<O, E>, CallError> {
        for tool in &self.all_tools {
            for function_name in tool.function_names() {
                if *function_name == tool_call.function_name {
                    return tool
                        .call(&tool_call.function_name, tool_call.parameters)
                        .await
                        .map_err(|err| err.into());
                }
            }
        }
        Err(CallError::FunctionNotFound {
            function_name: tool_call.function_name,
        })
    }

    fn str_split_into_tool_call(&self, input: &str) -> Result<ToolCall, CallParsingError> {
        let value =
            serde_json::from_str::<Value>(input)
                .ok()
                .ok_or_else(|| CallParsingError::Parsing {
                    issue: "The tool call is not valid json".to_owned(),
                })?;
        self.value_split_into_tool_call(value)
    }

    fn value_split_into_tool_call(&self, input: Value) -> Result<ToolCall, CallParsingError> {
        let name = match input.get("function_name") {
            Some(name) => name,
            None => {
                return Err(CallParsingError::Parsing {
                    issue: format!(
                        "The tool call is missing the `function_name` field in:\n{input}"
                    ),
                });
            }
        };
        let _ = match name.as_str() {
            Some(name) => name,
            None => {
                return Err(CallParsingError::Parsing {
                    issue: format!(
                        "The tool call `function_name` field is not a string in:\n{input}"
                    ),
                });
            }
        };
        let parameters = input.get("parameters");
        let Some(parameters) = parameters else {
            return Err(CallParsingError::Parsing {
                issue: format!("The tool call is missing the `parameters` field in:\n{input}"),
            });
        };
        if !parameters.is_object() {
            return Err(CallParsingError::Parsing {
                issue: format!("The tool call `parameters` field is not an object in:\n{input}"),
            });
        }
        let mut map = unwrap_match!(input, Value::Object);
        let name = map.remove("function_name").unwrap();
        let name = unwrap_match!(name, Value::String);
        let parameters = map.remove("parameters").unwrap();
        let parameters = unwrap_match!(parameters, Value::Object);
        return Ok(ToolCall { function_name: name, parameters });
    }

    //************************************************************************//

    pub fn schema(&self) -> &Map<String, Value> {
        &self.schema
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct ToolCall {
    function_name: String,
    parameters: Map<String, Value>,
}
