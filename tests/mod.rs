#[cfg(test)]
pub mod basic {
    //     use auto_toolbox::toolbox;
    //     //use llmtoolbox::toolbox;
    //     use serde_json::json;

    //     struct MyToolChest;

    //     #[toolbox] // this makes the MyToolChest struct into a toolbox giving it the `get_impl_json` associated function
    // impl MyToolChest {
    //     /// `bolt_location` - Location of bolt in need of tightening
    //     pub fn bolt_tightener(bolt_location: String) -> Result<String, std::io::Error> {
    //         // TODO add bolt tightening logic
    //         Ok(format!("I might have tightend the bolt located here: {}", bolt_location))
    //     }
    // }

    //     #[test]
    //     fn into_works_correctly() {

    //     }
}

#[cfg(test)]
pub mod toolbox {
    use std::{any::Any, cell::LazyCell};

    use jsonschema::Validator;
    use llmtoolbox::{Tool, ToolBox};
    use serde_json::{json, Map, Value};

    #[derive(Debug)]
    struct MyTool;

    impl MyTool {
        fn new() -> Self {
            Self
        }

        fn greet(&self, greeting: &str) -> String {
            format!("This is the greeting `{greeting}`")
        }
    }

    //************************************************************************//

    const MYTOOL_SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        Box::leak(Box::new(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "function name."
                },
                "args": {
                    "type": "object",
                    "description": "arguments for function",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to greet with"
                        }
                    },
                    "required": ["text"]
                }
            },
            "required": ["name", "args"]
        })))
    });

    const MYTOOL_VALIDATOR: LazyCell<&'static Validator> = LazyCell::new(|| {
        let schema = *MYTOOL_SCHEMA;
        Box::leak(Box::new(Validator::new(schema).expect(
            "The macro should not be able to create an invalid schema",
        )))
    });

    impl Tool<String> for MyTool {
        fn name(&self) -> &'static str {
            "greeting"
        }

        fn schema(&self) -> &'static serde_json::Map<String, serde_json::Value> {
            MYTOOL_SCHEMA.as_object().unwrap()
        }

        fn validator(&self) -> &'static jsonschema::Validator {
            *MYTOOL_VALIDATOR
        }

        fn run(
            &self,
            args: serde_json::Map<String, serde_json::Value>,
        ) -> Result<String, Box<dyn std::error::Error>> {
            let text = args
                .get("text")
                .ok_or("No greeting found")?
                .as_str()
                .ok_or("Not a string")?;
            Ok(self.greet(text))
        }
    }

    impl Tool<Box<dyn Any>> for MyTool {
        fn name(&self) -> &'static str {
            <Self as Tool<String>>::name(self)
        }

        fn schema(&self) -> &'static serde_json::Map<String, serde_json::Value> {
            <Self as Tool<String>>::schema(self)
        }

        fn validator(&self) -> &'static jsonschema::Validator {
            <Self as Tool<String>>::validator(self)
        }

        fn run(
            &self,
            args: serde_json::Map<String, serde_json::Value>,
        ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>> {
            <Self as Tool<String>>::run(self, args).map(|e| Box::new(e) as Box<dyn Any>)
        }
    }

    //************************************************************************//

    #[test]
    fn string_tool_works() {
        let mut toolbox: ToolBox<String> = ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let mut map = Map::new();
        map.insert("name".to_string(), Value::String("greeting".to_string()));
        let mut args = Map::new();
        args.insert(
            "text".to_string(),
            Value::String("This is a greeting".to_string()),
        );
        map.insert("args".to_string(), Value::Object(args));
        let tool_call_value: Value = Value::Object(map);
        let tool_call = toolbox.parse_value_tool_call(tool_call_value).unwrap();
        let message = toolbox.call(tool_call).unwrap();
        println!("End: {message}")
    }

    #[test]
    fn dyn_tool_works() {
        let mut toolbox: ToolBox<Box<dyn Any>> = ToolBox::new();
        toolbox.add_tool(MyTool::new()).unwrap();
        let mut map = Map::new();
        map.insert("name".to_string(), Value::String("greeting".to_string()));
        let mut args = Map::new();
        args.insert(
            "text".to_string(),
            Value::String("This is a greeting".to_string()),
        );
        map.insert("args".to_string(), Value::Object(args));
        let tool_call_value: Value = Value::Object(map);
        let tool_call = toolbox.parse_value_tool_call(tool_call_value).unwrap();
        let message = toolbox.call(tool_call).unwrap();
        match message.downcast::<String>() {
            Ok(message) => println!("End: {message}"),
            Err(_) => println!("Not a string"),
        }
    }
}
