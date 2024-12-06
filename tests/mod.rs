#[cfg(test)]
pub mod toolbox {
    use std::{any::Any, cell::LazyCell, collections::HashMap, convert::Infallible, error::Error};

    use jsonschema::Validator;
    use llmtoolbox::{Tool, ToolBox, ToolExecutionKey};
    use serde_json::{json, Map, Value};

    #[derive(Debug)]
    struct MyTool;

    // #[derive(LllmTool)]
    impl MyTool {
        fn new() -> Self {
            Self
        }

        // #[tool]
        fn greet(&self, greeting: &str) -> String {
            println!("Greetings!");
            format!("This is the greeting `{greeting}`")
        }

        // #[tool]
        fn goodbye(&self) -> u32 {
            println!("Goodbye!");
            0
        }
    }

    //************************************************************************//

    // https://platform.openai.com/docs/api-reference/runs/submitToolOutputs
    const MYTOOL_SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        Box::leak(Box::new(json!(
        {
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "greet",
                        "description": "",
                        "parameters": *MYTOOL_GREETING_PARAMETERS_SCHEMA
                    }
                },
                {
                    "type": "function",
                    "function": {
                        "name": "goodbye",
                        "description": "",
                        "parameters": *MYTOOL_GOODBYE_PARAMETERS_SCHEMA
                    }
                }
            ]
        }
        )))
    });

    const MYTOOL_GREETING_PARAMETERS_SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        Box::leak(Box::new(json!(
            {
                "type": "object",
                "properties": {
                    "greeting": {
                    "type": "string",
                    "description": "The greeting to give"
                    }
                },
                "required": ["greeting"]
            }
        )))
    });

    const MYTOOL_GOODBYE_PARAMETERS_SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        Box::leak(Box::new(json!(
            {
                "type": "object",
                "properties": {},
                "required": []
            }
        )))
    });

    // Note: Infallible since `greet` and `goodbye` do not return a result. `Box<dyn Any>` since
    // `greet` and `goodbye` have different return types
    #[async_trait::async_trait]
    impl Tool<Box<dyn Any>, Infallible> for MyTool {
        fn function_names(&self) -> &[&'static str] {
            &["greet", "goodbye"]
        }

        fn function_name_to_validator(&self) -> HashMap<&'static str, jsonschema::Validator> {
            let mut map = HashMap::new();
            const EXPECT_MSG: &str = "The macro should not be able to create an invalid schema";
            let schema = *MYTOOL_GREETING_PARAMETERS_SCHEMA;
            map.insert("greet", Validator::new(schema).expect(EXPECT_MSG));
            let schema = *MYTOOL_GOODBYE_PARAMETERS_SCHEMA;
            map.insert("goodbye", Validator::new(schema).expect(EXPECT_MSG));
            map
        }

        fn schema(&self) -> &'static Map<String, Value> {
            MYTOOL_SCHEMA.as_object().unwrap()
        }

        async fn run(&self, name: &str, parameters: &Map<String, Value>, _: &ToolExecutionKey) -> Result<Box<dyn Any>, Infallible> {
            const EXPECT_MSG: &str = "`ToolBox` should have validated parameters before calling `run`";
            match name {
                "greet" => {
                    let greet = parameters["greeting"].as_str().expect(EXPECT_MSG);
                    return Ok(Box::new(self.greet(greet)));
                }
                "goodbye" => {
                    return Ok(Box::new(self.goodbye()));
                }
                _ => unreachable!("`run` can only be called by `ToolBox` and `ToolBox` will never call `run` unless the function exists")
            };
        }
    }

    //************************************************************************//

    // #[test]
    // fn string_tool_works() {
    //     let mut toolbox: ToolBox<String, Box<dyn Error>> = ToolBox::new();
    //     toolbox.add_tool(MyTool::new()).unwrap();
    //     let mut map = Map::new();
    //     map.insert("name".to_string(), Value::String("greeting".to_string()));
    //     let mut parameters = Map::new();
    //     parameters.insert(
    //         "text".to_string(),
    //         Value::String("This is a greeting".to_string()),
    //     );
    //     map.insert("parameters".to_string(), Value::Object(parameters));
    //     let tool_call_value: Value = Value::Object(map);
    //     let tool_call = toolbox.parse_value_tool_call(tool_call_value).unwrap();
    //     let message = toolbox.call(tool_call).unwrap();
    //     println!("End: {message}")
    // }

    #[tokio::test]
    async fn dyn_tool_works() {
        let mut toolbox: ToolBox<Box<dyn Any>, Infallible> = ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let tool_call_value = json!({
            "name": "greet",
            "parameters": {
                "greeting": "This is a greeting"
            }
        });
        let tool_call = toolbox.parse_value_tool_call(tool_call_value);
        let tool_call = match tool_call {
            Ok(okay) => okay,
            Err(error) => panic!("{error}"),
        };
        let message = toolbox.call(&tool_call).await.unwrap();
        match message.downcast::<String>() {
            Ok(message) => println!("End: {message}"),
            Err(_) => println!("Not a string"),
        }
    }
}
