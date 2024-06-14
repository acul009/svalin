use proc_macro::TokenStream as ProcTokenStream;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma, SelfValue},
    Error, FnArg, ItemFn, Meta, Pat, PatType, Receiver, Signature,
};

#[proc_macro_attribute]
pub fn rpc_dispatch(args: ProcTokenStream, input: ProcTokenStream) -> ProcTokenStream {
    let args = parse_macro_input!(args with Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let item = parse_macro_input!(input as ItemFn);

    to_dispatcher(args, item).expect("should work...").into()
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

    if asyncness.is_none() {
        return Ok(parse_quote! {compile_error!("an RPC-Dispatcher needs to be async.")});
    }

    let mut trait_name = ident.to_string();
    trait_name.push_str("Dispatcher");
    let trait_ident = Ident::new(&trait_name, Span::call_site());

    let mut parameters: Punctuated<FnArg, Comma> = Punctuated::new();
    let mut iter = inputs.iter();

    let session = iter.next();

    for param in iter {
        parameters.push(param.to_owned())
    }

    let mut call_parameters: Punctuated<Box<Pat>, Comma> = Punctuated::new();
    let mut iter = inputs.iter();
    let _session = iter.next();

    for param in iter {
        match param {
            FnArg::Receiver(_) => unreachable!(),
            FnArg::Typed(pat) => call_parameters.push(pat.pat.to_owned()),
        }
    }

    let key_opt = args.first();

    match key_opt {
        Some(key) => Ok(syn::parse_quote!(
            async fn #ident(#inputs) #output {
                #block
            }

            #vis trait #trait_ident {
                async fn #ident(&self, #parameters) #output ;
            }

            impl<T> #trait_ident for T where T: svalin_rpc::rpc::connection::Connection {
                async fn #ident(&self, #parameters) #output {
                    let mut session = self.open_session(#key.to_owned()).await?;

                    let result = #ident(&mut session, #call_parameters).await;

                    // TODO
                    // somehow kills rpc tests?
                    // session.shutdown().await?;

                    result
                }
            }
        )),
        None => Ok(parse_quote! {compile_error!("an RPC-Dispatcher needs to have a handler.")}),
    }
}
