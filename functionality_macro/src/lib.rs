use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, ExprLit, ItemFn, Lit, MetaNameValue, Token, parse::Parser, parse_macro_input,
    punctuated::Punctuated,
};

#[proc_macro_attribute]
pub fn functionality(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();

    let input_fn = parse_macro_input!(item as ItemFn);

    if input_fn.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "functionality functions must take exactly one argument: Vec<String>",
        )
        .into_compile_error()
        .into();
    }

    if input_fn.sig.asyncness.is_some() {
        return syn::Error::new_spanned(&input_fn.sig, "async functions are not supported")
            .into_compile_error()
            .into();
    }

    let (name_expr, description_expr) = match parse_named_args(&attr_tokens)
        .or_else(|| parse_positional_args(&attr_tokens))
    {
        Some(values) => values,
        None => {
            return syn::Error::new_spanned(
                    &input_fn.sig.ident,
                    "use #[functionality(\"name\", \"description\")] or #[functionality(name = \"...\", description = \"...\")]",
                )
                .into_compile_error()
                .into();
        }
    };

    if !is_string_expr(&name_expr) {
        return syn::Error::new_spanned(name_expr, "name must be a string literal")
            .into_compile_error()
            .into();
    }

    if !is_string_expr(&description_expr) {
        return syn::Error::new_spanned(description_expr, "description must be a string literal")
            .into_compile_error()
            .into();
    }

    let fn_ident = &input_fn.sig.ident;
    let vis = &input_fn.vis;
    let strategy_ident = format_ident!("{}Strategy", to_pascal(&fn_ident.to_string()));
    let factory_ident = format_ident!("{}_functionality", fn_ident);

    let expanded = quote! {
        #input_fn

        #vis struct #strategy_ident;

        impl ::cli_core::CLIStrategy for #strategy_ident {
            fn execute(&self, args: Vec<String>) -> Result<(), ::cli_core::StrategyError> {
                #fn_ident(args)
            }
        }

        #vis fn #factory_ident() -> ::cli_core::Functionality {
            ::cli_core::Functionality {
                name: (#name_expr).to_string(),
                description: (#description_expr).to_string(),
                strategy: ::std::sync::Arc::new(#strategy_ident),
            }
        }
    };

    expanded.into()
}

fn is_string_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Lit(ExprLit {
            lit: Lit::Str(_),
            ..
        })
    )
}

fn parse_named_args(tokens: &proc_macro2::TokenStream) -> Option<(Expr, Expr)> {
    let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
    let args = parser.parse2(tokens.clone()).ok()?;

    let mut name_expr: Option<Expr> = None;
    let mut description_expr: Option<Expr> = None;

    for arg in args {
        if arg.path.is_ident("name") {
            name_expr = Some(arg.value);
        } else if arg.path.is_ident("description") {
            description_expr = Some(arg.value);
        }
    }

    Some((name_expr?, description_expr?))
}

fn parse_positional_args(tokens: &proc_macro2::TokenStream) -> Option<(Expr, Expr)> {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = parser.parse2(tokens.clone()).ok()?;
    if args.len() != 2 {
        return None;
    }

    let mut iter = args.into_iter();
    let name_expr = iter.next()?;
    let description_expr = iter.next()?;
    Some((name_expr, description_expr))
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
