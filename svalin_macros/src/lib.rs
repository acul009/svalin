use proc_macro::TokenStream;
use syn::{parse_macro_input, punctuated::Punctuated, Error, ItemFn, Meta, Signature};

#[proc_macro_attribute]
pub fn rpc_dispatch(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let item = parse_macro_input!(input as ItemFn);

    to_dispatcher(args, item);

    todo!()
}

fn to_dispatcher(
    args: Punctuated<Meta, syn::Token![,]>,
    item: ItemFn,
) -> Result<TokenStream, Error> {
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = item;
    let Signature {
        constness,
        asyncness,
        unsafety,
        abi,
        ident,
        generics,
        inputs,
        variadic,
        output,
        ..
    } = sig;

    todo!()
}
