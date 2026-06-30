//! `#[tool]` — turns a plain Rust function into an MCP tool: derives the JSON
//! Schema from the argument types + their doc comments, and generates a typed
//! dispatcher that deserializes `arguments-json` and calls the function. The
//! author writes Rust types, never JSON Schema, so the two cannot drift
//! (DESIGN §6).
//!
//! ```ignore
//! /// Fetch a pet by its ID.
//! #[tool]
//! pub fn get_pet(
//!     /// The pet's unique identifier.
//!     pet_id: u64,
//! ) -> Result<Json, ToolError> { /* ... */ }
//! ```
//!
//! Expands to (in addition to the cleaned `get_pet`):
//! `pub fn get_pet_tool() -> mcp_core::ToolHandle`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ItemFn, Pat, PatType, Type};

#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func = syn::parse_macro_input!(item as ItemFn);

    let fn_ident = func.sig.ident.clone();
    let name = fn_ident.to_string();
    let handle_ident = syn::Ident::new(&format!("{name}_tool"), fn_ident.span());
    let description = collect_docs(&func.attrs);

    // Build schema fragments + dispatch bindings, stripping the param doc
    // comments (rustc rejects them on params; we consume them here).
    let mut props = Vec::<String>::new();
    let mut required = Vec::<String>::new();
    let mut bindings = Vec::new();
    let mut call_args = Vec::new();

    for input in func.sig.inputs.iter_mut() {
        let arg: &mut PatType = match input {
            FnArg::Typed(p) => p,
            FnArg::Receiver(_) => {
                return compile_err(&fn_ident, "#[tool] functions cannot take `self`");
            }
        };
        let arg_name = match &*arg.pat {
            Pat::Ident(i) => i.ident.to_string(),
            _ => return compile_err(&fn_ident, "#[tool] arguments must be simple identifiers"),
        };
        let doc = collect_docs(&arg.attrs);
        arg.attrs.retain(|a| !a.path().is_ident("doc"));

        let (schema, optional) = type_schema(&arg.ty, &doc);
        props.push(format!("{}:{}", json_str(&arg_name), schema));
        if !optional {
            required.push(json_str(&arg_name));
        }

        let ident = syn::Ident::new(&arg_name, fn_ident.span());
        let ty = &arg.ty;
        bindings.push(quote! {
            let #ident: #ty = ::mcp_core::__rt::deserialize_arg(args, #arg_name)?;
        });
        call_args.push(quote! { #ident });
    }

    let schema = format!(
        "{{\"type\":\"object\",\"properties\":{{{}}},\"required\":[{}],\"additionalProperties\":false}}",
        props.join(","),
        required.join(",")
    );

    let expanded = quote! {
        #func

        #[allow(non_snake_case)]
        pub fn #handle_ident() -> ::mcp_core::ToolHandle {
            ::mcp_core::ToolHandle {
                name: #name,
                description: #description,
                input_schema: #schema,
                call: |args| {
                    #(#bindings)*
                    #fn_ident(#(#call_args),*)
                },
            }
        }
    };
    expanded.into()
}

/// Concatenate `#[doc = "..."]` attributes into a trimmed description.
fn collect_docs(attrs: &[syn::Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                lines.push(s.value().trim().to_string());
            }
        }
    }
    lines.join(" ").trim().to_string()
}

/// Map a Rust type to a JSON Schema fragment. Returns `(schema, optional)`,
/// where `optional` is true for `Option<T>` (excluded from `required`).
fn type_schema(ty: &Type, doc: &str) -> (String, bool) {
    if let Some(inner) = option_inner(ty) {
        let (schema, _) = type_schema(inner, doc);
        return (schema, true);
    }
    if let Some(inner) = vec_inner(ty) {
        // `items` already carries its own braces (it is a full schema object).
        let (items, _) = type_schema(inner, "");
        return (with_desc(format!("\"type\":\"array\",\"items\":{items}"), doc), false);
    }
    let base = match last_ident(ty).as_deref() {
        Some("u8") | Some("u16") | Some("u32") | Some("u64") | Some("u128") | Some("usize")
        | Some("i8") | Some("i16") | Some("i32") | Some("i64") | Some("i128") | Some("isize") => {
            "\"type\":\"integer\""
        }
        Some("f32") | Some("f64") => "\"type\":\"number\"",
        Some("bool") => "\"type\":\"boolean\"",
        Some("String") | Some("str") => "\"type\":\"string\"",
        _ => "\"type\":\"string\"",
    };
    (with_desc(base.to_string(), doc), false)
}

fn with_desc(body: String, doc: &str) -> String {
    if doc.is_empty() {
        format!("{{{body}}}")
    } else {
        format!("{{{body},\"description\":{}}}", json_str(doc))
    }
}

fn option_inner(ty: &Type) -> Option<&Type> {
    generic_inner(ty, "Option")
}
fn vec_inner(ty: &Type) -> Option<&Type> {
    generic_inner(ty, "Vec")
}

fn generic_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if seg.ident == wrapper {
            if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(t)) = ab.args.first() {
                    return Some(t);
                }
            }
        }
    }
    None
}

fn last_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(r) => last_ident(&r.elem),
        _ => None,
    }
}

/// Minimal JSON string literal (quotes + escapes).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn compile_err(span_src: &syn::Ident, msg: &str) -> TokenStream {
    syn::Error::new(span_src.span(), msg).to_compile_error().into()
}
