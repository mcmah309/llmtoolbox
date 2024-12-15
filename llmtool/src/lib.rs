use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use regex::Regex;
use syn::spanned::Spanned;
use syn::{parse_macro_input, GenericArgument, ItemImpl, PathArguments, Signature};
use syn::{FnArg, Ident, Pat, Type};

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
pub fn tool(
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

    if function_definitions.is_empty() {
        return syn::Error::new_spanned(
            struct_name,
            "No functions found in this tool. Please add functions to the tool with the `#[tool_part]` attribute.",
        )
        .into_compile_error()
        .into();
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

struct CommonReturnTypes<'a> {
    result_err: HashSet<&'a Type>,
    result_ok_and_regular: HashSet<&'a Type>,
}

impl<'a> CommonReturnTypes<'a> {
    pub fn new() -> Self {
        Self {
            result_err: HashSet::new(),
            result_ok_and_regular: HashSet::new(),
        }
    }
}

fn impl_traits(struct_name: &syn::Type, struct_name_str: &str, function_definitions: &Vec<FunctionDefintion>) -> TokenStream {
    let mut common_return_types = CommonReturnTypes::new();
    for function_definition in function_definitions.iter() {
        match &function_definition.return_type {
            ReturnType::Result(result_return_type) => {
                common_return_types.result_err.insert(&result_return_type.error);
                common_return_types.result_ok_and_regular.insert(&result_return_type.okay);
            }
            ReturnType::Other(other_return_type) => {
                common_return_types.result_ok_and_regular.insert(&other_return_type.other);
            }
        }
    }

    let mut common_err_type: Option<Type> = None;
    let all_are_results_with_same_err_type = common_return_types.result_err.len() == 1;
    if all_are_results_with_same_err_type {
        let first = *common_return_types.result_err.iter().next().unwrap();
        common_err_type = Some(first.clone());
    }
    let mut common_ok_type: Option<Type>  = None;
    let all_have_same_ok_type = common_return_types.result_ok_and_regular.len() == 1;
    if all_have_same_ok_type {
        let first = *common_return_types.result_ok_and_regular.iter().next().unwrap();
        common_ok_type = Some(first.clone());
    }

    let all_functions_are_regular = common_return_types.result_err.len() == 0; // aka no result functions
    let impls_needed = determine_impls_needed(common_ok_type, common_err_type, all_functions_are_regular);

    let mut all_impl_tokens = TokenStream::new();

    let box_any_type = quote! {
        Box<dyn std::any::Any>
    };
    let box_error_type = quote! {
        Box<dyn std::error::Error>
    };
    let infallible_type = quote! {
        std::convert::Infallible
    };
    for impl_needed in impls_needed {
        let tokens = match impl_needed {
            ImplTypes::BoxAndBox => impl_trait(struct_name, struct_name_str, function_definitions, true, true, &box_any_type, &box_error_type),
            ImplTypes::BoxAndSpecific(err_type) => impl_trait(struct_name, struct_name_str, function_definitions, true, false, &box_any_type, &err_type.to_token_stream()),
            ImplTypes::SpecificAndBox(ok_type) => impl_trait(struct_name, struct_name_str, function_definitions, false, true, &ok_type.to_token_stream(), &box_error_type),
            ImplTypes::SpecificAndSpecific(ok_type, err_type) => impl_trait(struct_name, struct_name_str, function_definitions, false, false, &ok_type.to_token_stream(), &err_type.to_token_stream()),
            ImplTypes::BoxAndInfallible => impl_trait(struct_name, struct_name_str, function_definitions, true, false, &box_any_type, &infallible_type),
            ImplTypes::SpecificAndInfallible(ok_type) => impl_trait(struct_name, struct_name_str, function_definitions, false, false, &ok_type.to_token_stream(), &infallible_type),
        };
        all_impl_tokens.append_all(tokens);
    }

    all_impl_tokens
}

enum ImplTypes {
    BoxAndBox,
    BoxAndSpecific(Type),
    SpecificAndBox(Type),
    SpecificAndSpecific(Type, Type),
    BoxAndInfallible,
    SpecificAndInfallible(Type),
}

fn determine_impls_needed(common_ok_type: Option<Type>, common_err_type: Option<Type>, all_functions_are_regular: bool) -> Vec<ImplTypes> {
    let mut vecs = match (common_ok_type.clone(), common_err_type.clone()) {
        (None, None) => vec![],
        (None, Some(err_type)) => vec![ImplTypes::BoxAndSpecific(err_type)],
        (Some(ok_type), None) => vec![ImplTypes::SpecificAndBox(ok_type)],
        (Some(ok_type), Some(err_type)) => vec![ImplTypes::BoxAndSpecific(err_type.clone()), ImplTypes::SpecificAndBox(ok_type.clone()), ImplTypes::SpecificAndSpecific(ok_type, err_type)],
    };
    if all_functions_are_regular {
        assert!(common_err_type.is_none(), "If there are no result functions, there should be no error type");
        vecs.push(ImplTypes::BoxAndInfallible);
        if let Some(common_ok_type) = common_ok_type {
            vecs.push(ImplTypes::SpecificAndInfallible(common_ok_type));
        }
    }
    vecs.push(ImplTypes::BoxAndBox);
    vecs
}

fn impl_trait(struct_name: &syn::Type, struct_name_str:&str, function_definitions: &Vec<FunctionDefintion>, ok_needs_box: bool, err_needs_box: bool, ok_type: &TokenStream, err_type: &TokenStream) -> TokenStream {
    let function_names = function_definitions.iter().map(|function_definition| {
        &function_definition.name_str
    });

    let run_arms = function_definitions.iter().map(|function_definition| {
        let function_parameter_statements = function_definition.parameters.iter().map(|parameter|{
            let Parameter {
                name,
                name_str,
                param_type,
                description: _,
            } = parameter;
            let serde_message = format!("Parameter `{}` does not follow schema", name_str);
            let missing_message = format!("Missing `{}` parameter", name_str);
            let deserialize= match param_type {
                Type::Reference(type_reference) => match &*type_reference.elem {
                    Type::Path(type_path) => {
                        if type_path.path.get_ident().is_some_and(|item| &*item.to_string() == "str") {
                            Some(quote! {
                                let #name: &str = &*serde_json::from_value::<String>(#name).map_err(|_| llmtoolbox::CallError::parsing(#serde_message.to_owned()))?;
                            })
                        }
                        else {
                            Some(quote! {
                                let #name: #param_type = &serde_json::from_value::<#type_path>(#name).map_err(|_| llmtoolbox::CallError::parsing(#serde_message.to_owned()))?;
                            })
                        }
                    },
                    _ => None,
                },
                _ => None,
            }.unwrap_or(quote! {
                let #name: #param_type = serde_json::from_value::<#param_type>(#name).map_err(|_| llmtoolbox::CallError::parsing(#serde_message.to_owned()))?;
            });
            quote! {
                let #name = parameters.remove(#name_str).ok_or_else(|| llmtoolbox::CallError::parsing(#missing_message.to_owned()))?;
                #deserialize
            }
        });
        let return_statement = 
        make_return_statement(function_definition, ok_needs_box, err_needs_box);
        let function_name_str = &function_definition.name_str;
        quote! {
            #function_name_str => {
                    #(#function_parameter_statements)*
                    #return_statement
                }
        }
    }).fold(TokenStream::new(), |mut acc, item| { acc.append_all(item); acc });

    let schema = create_tool_schema_const_indentifier(struct_name_str);
    quote! {
        //#[async_trait::async_trait]
        impl llmtoolbox::Tool<#ok_type, #err_type> for #struct_name {
            fn function_names(&self) -> &[&'static str] {
                &[
                    #(#function_names),*
                ]
            }

            fn schema(&self) -> &'static serde_json::Map<String, serde_json::Value> {
                #schema.as_object().unwrap()
            }

            fn call<'life0, 'life1, 'async_trait>(
                &'life0 self,
                name: &'life1 str,
                parameters: serde_json::Map<String, serde_json::Value>,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = Result<
                                Result<#ok_type, #err_type>,
                                llmtoolbox::CallError,
                            >,
                        > + ::core::marker::Send
                        + 'async_trait,
                >,
            >
            where
                'life0: 'async_trait,
                'life1: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async move {
                    if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<
                        Result<
                            Result<#ok_type, #err_type>,
                            llmtoolbox::CallError,
                        >,
                    > {
                        #[allow(unreachable_code)]
                        return __ret;
                    }
                    let __self = self;
                    let mut parameters = parameters;
                    let __ret: Result<
                        Result<#ok_type, #err_type>,
                        llmtoolbox::CallError,
                    > = {
                        match &*name {
                            #run_arms
                            _ => return Err(llmtoolbox::CallError::function_not_found(name.to_owned())),
                        }
                    };
                    #[allow(unreachable_code)]
                    __ret
                })
            }
            // async fn call(
            //     &self,
            //     name: &str,
            //     mut parameters: serde_json::Map<String, serde_json::Value>,
            // ) -> Result<Result<#ok_type, #err_type>, llmtoolbox::CallError> {
            //     match &*name {
            //         #run_arms
            //         _ => return Err(llmtoolbox::CallError::new(format!(
            //             "`{name}` is not a function in this tool"
            //         ))),
            //     }
            // }
        }
    }
}

fn make_return_statement(function_definition: &FunctionDefintion, ok_needs_box: bool, err_needs_box: bool) -> TokenStream {
    let async_part;
    if function_definition.is_async {
        async_part = quote! {
            .await
        }
    }
    else {
        async_part = quote! {}
    }
    let function_parameters = function_definition.parameters.iter().map(|parameter| {
        &parameter.name
    });
    let function_name = &function_definition.name;
    match function_definition.return_type {
        ReturnType::Result(_) => {
            if ok_needs_box {
                if err_needs_box {
                    quote! {
                        return Ok(match self.#function_name(#(#function_parameters),*)#async_part {
                            Ok(value) => Ok(Box::new(value) as Box<dyn std::any::Any>),
                            Err(value) => Err(Box::new(value) as Box<dyn std::error::Error>),
                        });
                    }
                }
                else {
                    quote! {
                        return Ok(self.#function_name(#(#function_parameters),*)#async_part.map(|value| Box::new(value) as Box<dyn std::any::Any>));
                    }
                }
            }
            else {
                if err_needs_box {
                    quote! {
                        return Ok(self.#function_name(#(#function_parameters),*)#async_part.map_err(|error| Box::new(error) as Box<dyn std::error::Error>));
                    }
                }
                else {
                    quote! {
                        return Ok(self.#function_name(#(#function_parameters),*)#async_part);
                    }
                }
            }
        },
        ReturnType::Other(_) => {
            if ok_needs_box {
                quote! {
                    return Ok(Ok(Box::new(self.#function_name(#(#function_parameters),*)#async_part)));
                }
            }
            else {
                quote! {
                    return Ok(Ok(self.#function_name(#(#function_parameters),*)#async_part));
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
                    // let type_str = arg.ty.to_token_stream().to_string();
                    let type_ = *arg.ty.clone();

                    Some(Parameter {
                        name,
                        name_str,
                        param_type: type_,
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
        is_async: _,
        name,
        name_str,
        parameters,
        return_type: _,
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
                    "type": "object",
                    "description": stringify!(#description),
                    "properties": {
                        "function_name": {
                            "const": stringify!(#name),
                        },
                        "parameters": *#id
                    },
                    "required": ["function_name", "parameters"]
                }
            )
        });
    }
    let id = create_tool_schema_const_indentifier(struct_name);
    quote! {
        const #id: std::cell::LazyCell<&'static serde_json::Value> = std::cell::LazyCell::new(|| {
            Box::leak(Box::new(serde_json::json!(
                {
                    "$schema": "http://json-schema.org/draft-07/schema#",
                    "oneOf": [    
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
                    let schema_settings = schemars::generate::SchemaSettings::draft07();
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
