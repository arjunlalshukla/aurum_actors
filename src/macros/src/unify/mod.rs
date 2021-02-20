
use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{self, Span};
use quote::{quote, ToTokens};
use syn::{Ident, Token, TypePath, punctuated::Punctuated};
use syn::parse::Parse;

pub fn unify_impl(toks: TokenStream) -> TokenStream {
  let UnifiedType {
    unified, 
    specifics
  } = syn::parse::<UnifiedType>(toks).unwrap();
  let mut specifics = specifics.into_iter()
    .map(|x| x.path.to_token_stream())
    .collect::<Vec<proc_macro2::TokenStream>>();
  specifics.push(quote!{
    aurum::core::RegistryMsg<#unified>
  });
  let variants = std::iter::repeat(('A'..='B').collect::<Vec<_>>())
    .take((specifics.len() as f64).log(1.9).ceil() as usize)
    .multi_cartesian_product()
    .take(specifics.len())
    .map(|x| Ident::new(x.into_iter().collect::<String>().as_str(), Span::call_site()))
    .collect::<Vec<_>>();
  let code = TokenStream::from(quote! {
    #[derive(serde::Serialize, serde::Deserialize, std::cmp::Eq,
      std::cmp::PartialEq, std::fmt::Debug, std::hash::Hash, std::clone::Clone
    )]
    enum #unified {
      #(#variants,)*
    }
    #(
      impl aurum::core::Case<#specifics> for #unified {
        const VARIANT: #unified = #unified::#variants;
      }
    )*
  });
  super::write_and_fmt(unified.to_string(), &code).expect("can't save codegen");
  TokenStream::new()
}

struct UnifiedType {
  unified: Ident,
  specifics: Vec<SpecificType>
}
impl Parse for UnifiedType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
      let unified = input.parse::<Ident>()?;
      input.parse::<Token![=]>()?;
      let specifics = Punctuated::<SpecificType, Token![|]>
        ::parse_terminated(&input)?
        .into_iter()
        .collect::<Vec<SpecificType>>();
      Ok(UnifiedType {
        unified: unified,
        specifics: specifics
      })
    }
}

struct SpecificType {
  path: TypePath
}
impl Parse for SpecificType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
      let path = input.parse::<TypePath>()?;
      Ok(SpecificType{path: path})
    }
}