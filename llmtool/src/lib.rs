use std::collections::{HashMap, HashSet};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use regex::Regex;
use syn::spanned::Spanned;
use syn::{parse_macro_input, GenericArgument, ItemImpl, LitStr, PathArguments, Signature};
use syn::{FnArg, Ident, ItemFn, Pat, Type};

fn create_tool_schema_const_indentifier(struct_name: &str) -> Ident {
    Ident::new(
        &format!("_{}_SCHEMA", struct_name.to_uppercase(),),
        Span::call_site(),
    )
}

struct FunctionDefintion {
    is_async: bool,
    name: Ident,
    name_str: String,
    parameters: Vec<Parameter>,
    return_type: ReturnType,
    // option because, late, but required
    description: Option<String>,
}

impl FunctionDefintion {
    fn create_schema_const_indentifier(&self, struct_name: &str) -> Ident {
        Ident::new(
            &format!(
                "_{}_{}_PARMETER_SCHEMA",
                struct_name.to_uppercase(),
                self.name_str.to_uppercase()
            ),
            Span::call_site(),
        )
    }
}

struct Parameter {
    name: Ident,
    name_str: String,
    param_type: syn::Type,
    type_str: String,
    // option because, late, but required
    description: Option<String>,
}

enum ReturnType {
    Result(ResultReturnType),
    Other(OtherReturnType),
}

struct ResultReturnType {
    okay: Type,
    error: Type,
}

struct OtherReturnType {
    other: Type,
}

#[proc_macro_attribute]
pub fn llmtool(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(item as ItemImpl);
    let struct_name = &input.self_ty;
    let struct_name_str = struct_name.to_token_stream().to_string();
    
    let methods: Vec<_> = input
        .items
        .clone()
        .into_iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(method) = item {
                let attrs = &method.attrs;
                for attr in attrs.iter() {
                    let path = attr.path();
                    if path.is_ident("tool_part") {
                        return Some(method);
                    }
                }
            }
            None
        })
        .collect();

    
    input
        .items
        .iter_mut()
        .for_each(|item| {
            if let syn::ImplItem::Fn(method) = item {
                method.attrs.retain(|attr|{
                    !attr.path().is_ident("tool_part")
                });
            }
        });


    let mut function_definitions = Vec::new();
    for method in methods {
        let syn::ImplItemFn {
            attrs,
            vis: _,
            defaultness: _,
            sig,
            block: _,
        } = method;
        let mut function_definition = match extract_function_defintion(sig) {
            Ok(okay) => okay,
            Err(error) => return error.into_compile_error().into(),
        };
        match extract_description(&mut function_definition, attrs) {
            Ok(_) => {}
            Err(error) => return error.into_compile_error().into(),
        }
        function_definitions.push(function_definition);
    }

    let function_schema = create_tool_json_schema(&struct_name_str, &mut function_definitions);
    let parameter_json_schema = function_definitions.iter_mut().map(|function_definition| {
        create_function_parameter_json_schema(&struct_name_str, function_definition)
    }).fold(TokenStream::new(), |mut acc, item| { acc.append_all(item); acc });

    let impl_traits = impl_traits(&struct_name, &struct_name_str, &function_definitions);

    let expanded = quote! {
        #input

        #function_schema

        #parameter_json_schema

        #impl_traits
    };

    proc_macro::TokenStream::from(expanded)
}

fn impl_traits(struct_name: &syn::Type, struct_name_str: &str, function_definitions: &Vec<FunctionDefintion>) -> TokenStream {
    let function_names = function_definitions.iter().map(|function_definition| {
        &function_definition.name_str
    });

    let function_name_to_validator = function_definitions.iter().map(|function_definition| {
        let name = &function_definition.name;
        let id = function_definition.create_schema_const_indentifier(struct_name_str);
        quote! {
            let schema = &*#id;
            map.insert(stringify!(#name), jsonschema::Validator::new(schema).expect(EXPECT_MSG));
        }
    }).fold(TokenStream::new(), |mut acc, item| { acc.append_all(item); acc });

    let run_arms = function_definitions.iter().map(|function_definition| {
        let function_parameter_statements = function_definition.parameters.iter().map(|parameter|{
            let Parameter {
                name,
                name_str,
                param_type,
                type_str:  _,
                description: _,
            } = parameter;
            let deserialize= match param_type {
                Type::Reference(type_reference) => match &*type_reference.elem {
                    Type::Path(type_path) => {
                        if type_path.path.get_ident().is_some_and(|item| &*item.to_string() == "str") {
                            Some(quote! {
                                let #name: &str = &*serde_json::from_value::<String>(#name).expect(EXPECT_MSG);
                            })
                        }
                        else {
                            Some(quote! {
                                let #name: #param_type = &serde_json::from_value::<#type_path>(#name).expect(EXPECT_MSG);
                            })
                        }
                    },
                    _ => None,
                },
                _ => None,
            }.unwrap_or(quote! {
                let #name: #param_type = serde_json::from_value::<#param_type>(#name).expect(EXPECT_MSG);
            });
            quote! {
                let #name = parameters.remove(#name_str).expect(EXPECT_MSG);
                #deserialize
            }
        });
        let function_parameters = function_definition.parameters.iter().map(|parameter| {
            &parameter.name
        });
        let function_name = &function_definition.name;
        let function_name_str = &function_definition.name_str;
        quote! {
            #function_name_str => {
                    #(#function_parameter_statements)*
                    return Ok(Box::new(self.#function_name(#(#function_parameters),*)));
                }
        }
    }).fold(TokenStream::new(), |mut acc, item| { acc.append_all(item); acc });

    let schema = create_tool_schema_const_indentifier(struct_name_str);
    quote! {
        #[async_trait::async_trait]
        impl llmtoolbox::Tool<Box<dyn std::any::Any>, std::convert::Infallible> for #struct_name {
            fn function_names(&self) -> &[&'static str] {
                &[
                    #(#function_names),*
                ]
            }

            fn function_name_to_validator(&self) -> std::collections::HashMap<&'static str, jsonschema::Validator> {
                let mut map = std::collections::HashMap::new();
                const EXPECT_MSG: &str = "The macro should not be able to create an invalid schema";
                #function_name_to_validator
                map
            }

            fn schema(&self) -> &'static serde_json::Map<String, serde_json::Value> {
                #schema.as_object().unwrap()
            }

            async fn run(
                &self,
                name: String,
                mut parameters: serde_json::Map<String, serde_json::Value>,
                _: &llmtoolbox::ToolExecutionKey,
            ) -> Result<Box<dyn std::any::Any>, std::convert::Infallible> {
                const EXPECT_MSG: &str = "`ToolBox` should have validated parameters before calling `run`";
                match &*name {
                    #run_arms
                    _ => panic!("`run` can only be called by `ToolBox` and `ToolBox` should never call `run` unless the function exists"),
                }
            }
        }
    }
}



fn extract_function_defintion(signature: Signature) -> syn::Result<FunctionDefintion> {
    let inputs = &signature.inputs;
    let parameters = inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(arg) = arg {
                if let Pat::Ident(pat_ident) = &*arg.pat {
                    let name_str = pat_ident.ident.to_string();
                    let name = pat_ident.ident.clone();
                    let type_str = arg.ty.to_token_stream().to_string();
                    let type_ = *arg.ty.clone();

                    Some(Parameter {
                        name,
                        name_str,
                        param_type: type_,
                        type_str,
                        description: None,
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let return_type = match signature.output {
        syn::ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                signature,
                "Currently, tool functions must have a return type, even if it is just `()`.",
            ))
        }
        syn::ReturnType::Type(_, return_type) => *return_type,
    };
    let return_type = (|| {
        match &return_type {
            Type::Path(type_path) => {
                let segments = &type_path.path.segments;
                if segments.len() != 1 {
                    return ReturnType::Other(OtherReturnType { other: return_type });
                }
                let segment = segments.last().unwrap();
                if let PathArguments::AngleBracketed(angle_bracketed_args) = &segment.arguments {
                    let mut generics = angle_bracketed_args.args.iter();

                    if let (Some(GenericArgument::Type(okay)), Some(GenericArgument::Type(error))) =
                        (generics.next(), generics.next())
                    {
                        return ReturnType::Result(ResultReturnType {
                            okay: okay.clone(),
                            error: error.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
        return ReturnType::Other(OtherReturnType { other: return_type });
    })();

    let is_async = signature.asyncness.is_some();
    let name = signature.ident;
    let name_str = name.to_string();
    Ok(FunctionDefintion {
        is_async,
        name,
        name_str,
        parameters,
        return_type,
        description: None,
    })
}

fn extract_description(
    function_definition: &mut FunctionDefintion,
    attrs: Vec<syn::Attribute>,
) -> syn::Result<()> {
    let FunctionDefintion {
        is_async,
        name,
        name_str,
        parameters,
        return_type,
        description,
    } = function_definition;
    let re = Regex::new(r".*?`(?<name>.*?)`\s*-\s*(?<description>.*)$").unwrap();
    for attr in attrs.iter() {
        match &attr.meta {
            syn::Meta::NameValue(name_value) => match &name_value.value {
                syn::Expr::Lit(lit) => match &lit.lit {
                    syn::Lit::Str(str) => {
                        let haystack = str.value();
                        let arg_caps = match re.captures(&haystack) {
                            Some(caps) => caps,
                            None => {
                                if let Some(description) = description {
                                    description.push_str(&*format!("{}\n", &str.value().trim()));
                                } else {
                                    let _ = description.insert(str.value().trim().to_string());
                                }
                                continue;
                            }
                        };
                        let name = arg_caps["name"].to_string();
                        let desc = arg_caps["description"].to_string();
                        if let Some(param) = parameters.iter_mut().find(|p| p.name_str == name) {
                            param.description = Some(desc);
                        } else {
                            return Err(syn::Error::new_spanned(
                                attr,
                                format!("parameter `{}` not found in function definition", name),
                            ));
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
    }
    for parameter in parameters {
        if parameter.description.is_none() {
            return Err(syn::Error::new_spanned(
                parameter.name.clone(),
                format!("missing description for parameter `{}`. Descriptions are doc comments the form of:\n\
                /// `parameter_name` - This is the description for the parameter.", parameter.name_str),
            ));
        }
    }
    if function_definition.description.is_none() {
        return Err(syn::Error::new_spanned(
            name.clone(),
            format!("missing description for function `{}`", name_str),
        ));
    }
    Ok(())
}

/// Attempt to determine the correct json schema type at compile time, that is not an object
fn rust_type_to_known_json_schema_type(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                return match segment.ident.to_string().as_str() {
                    "String" | "str" => Some("string"),
                    // json_serde only support `i64`, `u64`, `f64` as a final result
                    "i8" | "i16" | "i32" | "i64" | "isize" => Some("integer"),
                    "u8" | "u16" | "u32" | "u64" | "usize" => Some("integer"), // todo if u, add to description it needs to b unsigned.
                    "u128" | "i128" => Some("integer"), // todo compile_error!("json_serde only support `i64`, `u64`, `f64` as a final result. The the type needs to be compatible."),
                    "f32" | "f64" => Some("number"),
                    "bool" => Some("boolean"),
                    _ => None,
                };
            } else {
                None
            }
        }
        Type::Reference(type_ref) => rust_type_to_known_json_schema_type(&type_ref.elem),
        _ => None,
    }
}

fn create_tool_json_schema(
    struct_name: &str,
    function_definitions: &Vec<FunctionDefintion>,
) -> proc_macro2::TokenStream {
    let mut function_schemas = Vec::new();
    for function_definition in function_definitions {
        let id = function_definition.create_schema_const_indentifier(struct_name);
        let description = &function_definition.description;
        let name = &function_definition.name;

        function_schemas.push(quote! {
            serde_json::json!(
                {
                    "type": "function",
                    "function": {
                        "name": stringify!(#name),
                        "description": stringify!(#description),
                        "parameters": *#id
                    }
                }
            )
        });
    }
    let id = create_tool_schema_const_indentifier(struct_name);
    quote! {
        const #id: std::cell::LazyCell<&'static serde_json::Value> = std::cell::LazyCell::new(|| {
            Box::leak(Box::new(serde_json::json!(
                {
                    "tools": [
                        #(#function_schemas),*
                    ]
                }
            )))
        });
    }
}

fn create_function_parameter_json_schema(
    struct_name: &str,
    function_definition: &mut FunctionDefintion,
) -> proc_macro2::TokenStream {
    let parameters = &function_definition.parameters;
    let mut known_properties = Vec::new();
    let mut known_required_property_name = Vec::new();
    let mut computed_required_property_name = Vec::new();
    // definition of the variable used in `computed_properties`
    let mut computed_properties_outer_definitions = Vec::new();
    let mut computed_properties = Vec::new();
    let mut num_of_computed_properties = 0;
    for parameter in parameters {
        let name = &parameter.name_str;
        let description = &parameter.description;
        let param_type = &parameter.param_type;
        let json_schema_type = rust_type_to_known_json_schema_type(&parameter.param_type);
        if let Some(param_type) = json_schema_type {
            known_properties.push(quote! {
                #name: {
                    "type": #param_type,
                    "description": #description
                }
            });
            known_required_property_name.push(quote! {
                #name
            });
        } else {
            num_of_computed_properties +=1;
            let id = Ident::new(
                &format!("computed{num_of_computed_properties}"),
                json_schema_type.span(),
            );
            computed_properties_outer_definitions.push(quote! {
                let #id = (|| {
                    let schema_settings = schemars::generate::SchemaSettings::draft2020_12();
                    let schema = schemars::SchemaGenerator::new(schema_settings).into_root_schema_for::<#param_type>();
                    let mut schema = schema.to_value();
                    llmtoolbox::clean_up_schema(&mut schema);
                    match schema {
                        serde_json::Value::Object(ref mut map) => { 
                            map.insert("description".to_string(), serde_json::Value::String(#description.to_string())); 
                        },
                        _ => panic!("schema should always generate a map type.")
                    }
                    return schema;
                })();
            });
            computed_properties.push(quote! {
                #name: #id
            });
            computed_required_property_name.push(quote! {
                #name
            });
        }
    }
    let id = function_definition.create_schema_const_indentifier(struct_name);
    quote! {
        const #id: std::cell::LazyCell<serde_json::Value> = std::cell::LazyCell::new(|| {
            #(#computed_properties_outer_definitions)*
            serde_json::json!(
                {
                    "type": "object",
                    "required": [
                        #(#known_required_property_name),*
                        #(#computed_required_property_name),*
                    ],
                    "properties": {
                        #(#known_properties),*
                        #(#computed_properties),*
                    },
                }
            )
        });
    }
}
