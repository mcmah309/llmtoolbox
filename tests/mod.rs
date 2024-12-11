#[cfg(test)]
pub mod toolbox_by_hand {
    use std::{any::Any, cell::LazyCell, convert::Infallible};

    use llmtoolbox::{CallError, Tool, ToolBox};
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

        fn schema(&self) -> &'static Map<String, Value> {
            _MYTOOL_SCHEMA.as_object().unwrap()
        }

        async fn call(
            &self,
            name: &str,
            mut parameters: Map<String, Value>,
        ) -> Result<Result<Box<dyn Any>, Infallible>, CallError> {
            match &*name {
                "greet" => {
                    let greeting = parameters
                        .remove("greeting")
                        .ok_or_else(|| CallError::new("Missing `greeting` param".to_owned()))?;
                    let greeting: &str = &*serde_json::from_value::<String>(greeting)
                        .ok()
                        .ok_or_else(|| {
                            CallError::new("`greeting` param does not follow schema ...".to_owned())
                        })?;
                    return Ok(Ok(Box::new(self.greet(&greeting))));
                }
                "goodbye" => {
                    return Ok(Ok(Box::new(self.goodbye())));
                }
                _ => {
                    return Err(CallError::new(format!(
                        "`{name}` is not a function in this tool"
                    )))
                }
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
        let message = match toolbox.call(tool_call_value).await {
            Ok(Ok(tool_result)) => tool_result,
            Err(error) => panic!("{error}"),
        };
        match message.downcast::<String>() {
            Ok(message) => println!("End: {message}"),
            Err(_) => println!("Not a string"),
        }
    }
}

#[cfg(test)]
pub mod toolbox_different_regular_return_type {

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
        fn greet(&self, greeting: &str) -> String {
            println!("Greetings!");
            format!("This is the greeting `{greeting}`")
        }

        fn goodbye(&self) -> u32 {
            println!("Goodbye!");
            1
        }

        /// func descrip
        /// `topic` - field description
        #[tool_part]
        async fn talk(&self, topic: ConverstationTopic) -> u32 {
            let ConverstationTopic { topic, opinion } = topic;
            println!("For {topic} it is {opinion}");
            0
        }
    }

    /// Description
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    pub struct ConverstationTopic {
        pub topic: String,
        pub opinion: String,
    }

    #[tokio::test]
    async fn test_it() {
        let mut toolbox: llmtoolbox::ToolBox<Box<dyn std::any::Any>, Box<dyn std::any::Any>> =
            llmtoolbox::ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let mut toolbox: llmtoolbox::ToolBox<Box<dyn std::any::Any>, std::convert::Infallible> =
            llmtoolbox::ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let tool_call_value = serde_json::json!({
            "name": "greet",
            "parameters": {
                "greeting": "This is a greeting"
            }
        });
        let message = match toolbox.call(tool_call_value).await {
            Ok(Ok(tool_result)) => tool_result,
            Err(error) => panic!("{error}"),
        };
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

#[cfg(test)]
pub mod toolbox_same_regular_return_type {

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
        fn greet(&self, greeting: &str) -> String {
            println!("Greetings!");
            format!("This is the greeting `{greeting}`")
        }

        fn goodbye(&self) -> u32 {
            println!("Goodbye!");
            1
        }

        /// func descrip
        /// `topic` - field description
        #[tool_part]
        async fn talk(&self, topic: ConverstationTopic) -> String {
            let ConverstationTopic { topic, opinion } = topic;
            println!("For {topic} it is {opinion}");
            String::new()
        }
    }

    /// Description
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    pub struct ConverstationTopic {
        pub topic: String,
        pub opinion: String,
    }

    #[tokio::test]
    async fn test_it() {
        let mut toolbox: llmtoolbox::ToolBox<Box<dyn std::any::Any>, Box<dyn std::any::Any>> =
            llmtoolbox::ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let mut toolbox: llmtoolbox::ToolBox<Box<dyn std::any::Any>, std::convert::Infallible> =
            llmtoolbox::ToolBox::new();
            let mut toolbox: llmtoolbox::ToolBox<String, Box<dyn std::any::Any>> =
            llmtoolbox::ToolBox::new();
        let mut toolbox: llmtoolbox::ToolBox<String, std::convert::Infallible> =
            llmtoolbox::ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let tool_call_value = serde_json::json!({
            "name": "greet",
            "parameters": {
                "greeting": "This is a greeting"
            }
        });
        let message = match toolbox.call(tool_call_value).await {
            Ok(Ok(tool_result)) => tool_result,
            Err(error) => panic!("{error}"),
        };
        println!("End: {message}");
    }
}
