extern crate proc_macro;
use proc_macro::TokenStream;

mod aurum_interface;
mod unify;

use aurum_interface::RootImpl;

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
  RootImpl::derive(syn::parse(item).unwrap())
    .map(|x| x.expand())
    .unwrap_or_else(|x| x.to_compile_error().into())
}

#[proc_macro]
pub fn unify(item: TokenStream) -> TokenStream {
  unify::unify_impl(item)
}
