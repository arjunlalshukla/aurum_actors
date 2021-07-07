extern crate proc_macro;
use proc_macro::TokenStream;

mod aurum_interface;
mod unify;

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
  aurum_interface::aurum_interface_impl(syn::parse(item).unwrap())
}

#[proc_macro]
pub fn unify(item: TokenStream) -> TokenStream {
  unify::unify_impl(item)
}
