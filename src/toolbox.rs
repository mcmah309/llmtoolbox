use serde_json::{Map, Value};

use crate::{utils::unwrap_match, FunctionCallError, FunctionCallParsingError, Tool};

/// A toolbox is a collection of tools that can be called by name with arguments.
pub struct ToolBox<T> where T: Tool + 'static {
    /// all the tools that the llm can call
    all_tools: Vec<Box<T>>,
    /// schema to be sent to the llm
    schema: Map<String, Value>,
}

impl<T> ToolBox<T> where T: Tool + 'static {
    pub fn new() -> Self {
        Self {
            all_tools: Vec::new(),
            schema: Map::new(),
        }
    }

    // todo add merge to allow merging toolboxes across crates

    /// Adds the `tool` to this [`Toolbox`]. If a tool with the same name already exists, will return
    /// Err with the tool.
    pub fn add_tool(&mut self, tool: T) -> Result<(), T> {
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
    pub async fn call_from_value(&self, function_call: Value) -> Result<Result<T::Output, T::Error>, FunctionCallError> {
        let function_call = self.into_function_call_from_value(function_call)?;
        self.call_from_args(function_call).await
    }

    /// Calls the tool with the given name and parameters.
    pub async fn call_from_str(&self, function_call: &str) -> Result<Result<T::Output, T::Error>, FunctionCallError> {
        let function_call = self.into_function_call_from_str(function_call)?;
        self.call_from_args(function_call).await
    }

    pub async fn call_from_args(&self, function_call: FunctionCallArgs) -> Result<Result<T::Output, T::Error>, FunctionCallError> {
        for tool in &self.all_tools {
            for function_name in tool.function_names() {
                if *function_name == function_call.function_name {
                    return tool
                        .call_function(&function_call.function_name, function_call.parameters)
                        .await
                        .map_err(|err| err.into());
                }
            }
        }
        Err(FunctionCallError::FunctionNotFound {
            function_name: function_call.function_name,
        })
    }

    pub fn into_function_call_from_str(&self, input: &str) -> Result<FunctionCallArgs, FunctionCallParsingError> {
        let value =
            serde_json::from_str::<Value>(input)
                .ok()
                .ok_or_else(|| FunctionCallParsingError::Parsing {
                    issue: "The tool call is not valid json".to_owned(),
                })?;
        self.into_function_call_from_value(value)
    }

    pub fn into_function_call_from_value(&self, input: Value) -> Result<FunctionCallArgs, FunctionCallParsingError> {
        let name = match input.get("function_name") {
            Some(name) => name,
            None => {
                return Err(FunctionCallParsingError::Parsing {
                    issue: format!(
                        "The tool call is missing the `function_name` field in:\n{input}"
                    ),
                });
            }
        };
        let _ = match name.as_str() {
            Some(name) => name,
            None => {
                return Err(FunctionCallParsingError::Parsing {
                    issue: format!(
                        "The tool call `function_name` field is not a string in:\n{input}"
                    ),
                });
            }
        };
        let parameters = input.get("parameters");
        let Some(parameters) = parameters else {
            return Err(FunctionCallParsingError::Parsing {
                issue: format!("The tool call is missing the `parameters` field in:\n{input}"),
            });
        };
        if !parameters.is_object() {
            return Err(FunctionCallParsingError::Parsing {
                issue: format!("The tool call `parameters` field is not an object in:\n{input}"),
            });
        }
        let mut map = unwrap_match!(input, Value::Object);
        let name = map.remove("function_name").unwrap();
        let name = unwrap_match!(name, Value::String);
        let parameters = map.remove("parameters").unwrap();
        let parameters = unwrap_match!(parameters, Value::Object);
        return Ok(FunctionCallArgs { function_name: name, parameters });
    }

    //************************************************************************//

    pub fn schema(&self) -> &Map<String, Value> {
        &self.schema
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct FunctionCallArgs {
    function_name: String,
    parameters: Map<String, Value>,
}
