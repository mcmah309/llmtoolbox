#[cfg(test)]
pub mod toolbox_by_hand {
    use std::{any::Any, cell::LazyCell, collections::HashMap, convert::Infallible};

    use jsonschema::Validator;
    use llmtoolbox::{Tool, ToolBox, ToolExecutionKey};
    use serde_json::{json, Map, Value};

    #[derive(Debug)]
    struct MyTool;

    impl MyTool {
        fn new() -> Self {
            Self
        }

        fn greet(&self, greeting: &str) -> String {
            println!("Greetings!");
            format!("This is the greeting `{greeting}`")
        }

        fn goodbye(&self) -> u32 {
            println!("Goodbye!");
            0
        }
    }

    //************************************************************************//

    // https://platform.openai.com/docs/api-reference/runs/submitToolOutputs
    const _MYTOOL_SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        Box::leak(Box::new(json!(
        {
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "greet",
                        "description": "",
                        "parameters": *_MYTOOL_GREETING_PARAMETERS_SCHEMA
                    }
                },
                {
                    "type": "function",
                    "function": {
                        "name": "goodbye",
                        "description": "",
                        "parameters": *_MYTOOL_GOODBYE_PARAMETERS_SCHEMA
                    }
                }
            ]
        }
        )))
    });

    const _MYTOOL_GREETING_PARAMETERS_SCHEMA: LazyCell<serde_json::Value> = LazyCell::new(|| {
        json!(
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
        )
    });

    const _MYTOOL_GOODBYE_PARAMETERS_SCHEMA: LazyCell<serde_json::Value> = LazyCell::new(|| {
        json!(
            {
                "type": "object",
                "properties": {},
                "required": []
            }
        )
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
            let schema = &*_MYTOOL_GREETING_PARAMETERS_SCHEMA;
            map.insert("greet", Validator::new(schema).expect(EXPECT_MSG));
            let schema = &*_MYTOOL_GOODBYE_PARAMETERS_SCHEMA;
            map.insert("goodbye", Validator::new(schema).expect(EXPECT_MSG));
            map
        }

        fn schema(&self) -> &'static Map<String, Value> {
            _MYTOOL_SCHEMA.as_object().unwrap()
        }

        async fn run(
            &self,
            name: String,
            mut parameters: Map<String, Value>,
            _: &ToolExecutionKey,
        ) -> Result<Box<dyn Any>, Infallible> {
            const EXPECT_MSG: &str =
                "`ToolBox` should have validated parameters before calling `run`";
            match &*name {
                "greet" => {
                    let greeting = parameters.remove("greeting").expect(EXPECT_MSG);
                    let greeting: String = serde_json::from_value(greeting).expect(EXPECT_MSG);
                    return Ok(Box::new(self.greet(&greeting)));
                }
                "goodbye" => {
                    return Ok(Box::new(self.goodbye()));
                }
                _ => unreachable!("`run` can only be called by `ToolBox` and `ToolBox` will never call `run` unless the function exists")
            };
        }
    }

    //************************************************************************//

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
        let message = toolbox.call(tool_call).await.unwrap();
        match message.downcast::<String>() {
            Ok(message) => println!("End: {message}"),
            Err(_) => println!("Not a string"),
        }
    }
}

#[cfg(test)]
pub mod toolbox_with_macro {

    #[derive(Debug)]
    struct MyTool;

    #[llmtool::llmtool]
    impl MyTool {
        fn new() -> Self {
            Self
        }

        /// This
        /// `greeting` - descr
        #[tool_part]
        fn greet(&self, greeting: String) -> String {
            println!("Greetings!");
            format!("This is the greeting `{greeting}`")
        }

        // #[tool_part]
        fn goodbye(&self) -> u32 {
            println!("Goodbye!");
            0
        }

        /// func descrip
        /// `my_struct` - field description
        #[tool_part]
        fn goodbye2(&self, my_struct: MyStruct) -> u32 {
            println!("Goodbye!");
            0
        }
    }

    /// Description
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    pub struct MyStruct {
        pub my_int: i32,
        pub my_bool: bool,
        // pub my_nullable_enum: Option<MyEnum>,
    }

    #[derive(serde::Deserialize, schemars::JsonSchema)]
    pub enum MyEnum {
        /// This is a description
        StringNewType(String),
        StructVariant {
            floats: Vec<f32>,
        },
    }

    #[tokio::test]
    async fn dyn_tool_works() {
        let mut toolbox: llmtoolbox::ToolBox<Box<dyn std::any::Any>, std::convert::Infallible> = llmtoolbox::ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let tool_call_value = serde_json::json!({
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
        let message = toolbox.call(tool_call).await.unwrap();
        match message.downcast::<String>() {
            Ok(message) => println!("End: {message}"),
            Err(_) => println!("Not a string"),
        }
        // let schema = &*_MYTOOL_GOODBYE2_PARMETER_SCHEMA;
        // let schema = &*_MYTOOL_SCHEMA;
        // let schema = serde_json::to_string_pretty(&schema).unwrap();
        // println!("{}", schema);
    }
}
