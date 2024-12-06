use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use regex::Regex;
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
    type_: syn::Type,
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
    let input = parse_macro_input!(item as ItemImpl);
    let ty = &input.self_ty;
    let struct_name = ty.to_token_stream().to_string();

    let methods: Vec<_> = input
        .items
        .into_iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(method) = item {
                let attrs = &method.attrs;
                for attr in attrs.iter() {
                    let path = attr.path();
                    if path.is_ident("add") {
                        return Some(method);
                    }
                }
            }
            return None;
        })
        .collect();

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

    let schema_const_indentifier = create_tool_json_schema(&struct_name, &mut function_definitions);
    let parameter_json_schema_temp = function_definitions.iter_mut().map(|function_definition| {
        create_function_parameter_json_schema(&struct_name, function_definition)
    });
    let mut parameter_json_schema = TokenStream::new();
    parameter_json_schema.append_all(parameter_json_schema_temp);

    let expanded = quote! {
        #schema_const_indentifier

        #parameter_json_schema
    };

    proc_macro::TokenStream::from(expanded)
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
                        type_,
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

fn rust_type_to_json_schema(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let ret = match segment.ident.to_string().as_str() {
                    "String" => "string",
                    // json_serde only support `i64`, `u64`, `f64` as a final result
                    "i8" | "i16" | "i32" | "i64" | "isize" => "integer",
                    "u8" | "u16" | "u32" | "u64" | "usize" => "integer", // todo if u, add to description it needs to b unsigned.
                    "u128" | "i128" => "integer", // todo compile_error!("json_serde only support `i64`, `u64`, `f64` as a final result. The the type needs to be compatible."),
                    "f32" | "f64" => "number",
                    "bool" => "boolean",
                    _ => "object", // todo handle
                };
                ret.to_string()
            } else {
                "object".to_string() // todo handle
            }
        }
        Type::Reference(type_ref) => rust_type_to_json_schema(&type_ref.elem),
        _ => "object".to_string(), // todo handle
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
    let mut fields = Vec::new();
    let mut required = Vec::new();
    for parameter in parameters {
        let name = &parameter.name_str;
        let description = &parameter.description;
        let type_ = rust_type_to_json_schema(&parameter.type_);
        fields.push(quote! {
            #name: {
                "type": #type_,
                "description": #description
            }
        });
        required.push(quote! {
            #name
        });
    }
    let id = function_definition.create_schema_const_indentifier(struct_name);
    quote! {
        const #id: std::cell::LazyCell<serde_json::Value> = std::cell::LazyCell::new(|| {
            serde_json::json!(
                {
                    "type": "object",
                    "required": [#(#required),*],
                    "properties": {#(#fields),*},
                }
            )
        });
    }
}
