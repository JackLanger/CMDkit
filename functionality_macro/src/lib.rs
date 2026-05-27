use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn cli(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    if !attr_tokens.is_empty() {
        return syn::Error::new_spanned(attr_tokens, "#[cli] does not take arguments")
            .into_compile_error()
            .into();
    }

    let input_fn = parse_macro_input!(item as ItemFn);

    if !input_fn.sig.inputs.is_empty() {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "cli strategy functions must not take arguments",
        )
        .into_compile_error()
        .into();
    }

    if input_fn.sig.asyncness.is_some() {
        return syn::Error::new_spanned(&input_fn.sig, "async functions are not supported")
            .into_compile_error()
            .into();
    }

    let fn_ident = &input_fn.sig.ident;
    let vis = &input_fn.vis;
    let strategy_ident = format_ident!("{}", to_pascal(&fn_ident.to_string()));
    let factory_ident = format_ident!("{}_strategy", fn_ident);

    let expanded = quote! {
        #input_fn

        #vis struct #strategy_ident;

        impl #strategy_ident {
            #vis fn new() -> Self {
                Self
            }
        }

        impl ::cli_core::CommandStrategy for #strategy_ident {
            fn execute(&self, _args: Vec<String>) -> Result<(), ::cli_core::StrategyError> {
                #fn_ident()
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
