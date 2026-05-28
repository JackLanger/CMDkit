use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, PatType, Receiver, ReturnType, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn cli(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();

    if !attr_tokens.is_empty() {
        return syn::Error::new_spanned(attr_tokens, "cli attribute does not take any arguments")
            .into_compile_error()
            .into();
    }

    let input_fn = parse_macro_input!(item as ItemFn);

    if input_fn.sig.asyncness.is_some() {
        return syn::Error::new_spanned(&input_fn.sig, "async functions are not supported")
            .into_compile_error()
            .into();
    }

    let mut inputs = input_fn.sig.inputs.iter();
    match inputs.next() {
        Some(FnArg::Receiver(Receiver {
            reference: Some(_),
            mutability: _,
            attrs,
            ..
        })) if attrs.is_empty() => {}
        Some(FnArg::Receiver(_)) => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "cli strategy methods must use an attribute-free &self receiver",
            )
            .into_compile_error()
            .into();
        }
        _ => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "cli strategy functions must match CommandStrategy::execute with an &self receiver and Vec<String> arguments",
            )
            .into_compile_error()
            .into();
        }
    }

    let pat = match inputs.next() {
        Some(FnArg::Typed(PatType { pat, ty, .. })) => {
            if inputs.next().is_some() {
                return syn::Error::new_spanned(
                    &input_fn.sig,
                    "cli strategy functions must accept exactly one Vec<String> argument",
                )
                .into_compile_error()
                .into();
            }

            match ty.as_ref() {
                Type::Path(path)
                    if path.path.segments.len() == 1 && path.path.segments[0].ident == "Vec" => {}
                _ => {
                    return syn::Error::new_spanned(
                        ty,
                        "cli strategy functions must accept Vec<String> arguments",
                    )
                    .into_compile_error()
                    .into();
                }
            }

            pat
        }
        _ => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "cli strategy functions must accept a Vec<String> argument",
            )
            .into_compile_error()
            .into();
        }
    };

    match &input_fn.sig.output {
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(path)
                if path.path.segments.len() == 1 && path.path.segments[0].ident == "Result" => {}
            _ => {
                return syn::Error::new_spanned(
                    ty,
                    "cli strategy functions must return Result<(), cli_core::StrategyError>",
                )
                .into_compile_error()
                .into();
            }
        },
        ReturnType::Default => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "cli strategy functions must return Result<(), cli_core::StrategyError>",
            )
            .into_compile_error()
            .into();
        }
    }

    let fn_ident = &input_fn.sig.ident;
    let vis = &input_fn.vis;
    let strategy_ident = format_ident!("{}", to_pascal(&fn_ident.to_string()));
    let factory_ident = format_ident!("{}_strategy", fn_ident);
    let attrs = &input_fn.attrs;
    let body = &input_fn.block;

    let expanded = quote! {
        #(#attrs)*
        #vis struct #strategy_ident;

        impl #strategy_ident {
            #vis fn new() -> Self {
                Self
            }
        }

        impl ::cli_core::CommandStrategy for #strategy_ident {
            fn execute(&self, #pat: Vec<String>) -> Result<(), ::cli_core::StrategyError> {
                #body
            }
        }

        #vis fn #factory_ident() -> #strategy_ident {
            #strategy_ident::new()
        }
    };

    expanded.into()
}

fn to_pascal(s: &str) -> String {
    let mut out = String::new();
    for part in s.split('_') {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}
