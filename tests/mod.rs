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

    const SCHEMA: LazyCell<&'static serde_json::Value> = LazyCell::new(|| {
        // Box::leak(Box::new(json!({
        //     "type": "function",
        //     "function": {
        //         "name": "greeting",
        //         "description": "greets someone",
        //         "parameters": {
        //             "text": {
        //                 "type": "string",
        //                 "description": "The text to greet with"
        //             },
        //         },
        //         "required": ["text"]
        //     }
        // })))
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

    const VALIDATOR: LazyCell<&'static Validator> = LazyCell::new(|| {
        let schema = *SCHEMA;
        Box::leak(Box::new(Validator::new(schema).expect(
            "The macro should not be able to create an invalid schema",
        )))
    });

    impl Tool<Box<dyn Any>> for MyTool {
        fn name(&self) -> &'static str {
            "greeting"
        }

        fn schema(&self) -> &'static serde_json::Map<String, serde_json::Value> {
            let x = SCHEMA.as_object().unwrap();
            x
        }

        fn validator(&self) -> &'static jsonschema::Validator {
            *VALIDATOR
        }

        fn run(
            &self,
            args: serde_json::Map<String, serde_json::Value>,
        ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>> {
            let text = args
                .get("text")
                .ok_or("No greeting found")?
                .as_str()
                .ok_or("Not a string")?;
            Ok(Box::new(self.greet(text)))
        }

        //************************************************************************//
    }

    #[test]
    fn into_works_correctly() {
        let mut toolbox = ToolBox::new();
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
