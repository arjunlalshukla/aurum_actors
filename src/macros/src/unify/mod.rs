use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{self, Span};
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syn::{punctuated::Punctuated, Ident, Token, TypePath};

pub fn unify_impl(toks: TokenStream) -> TokenStream {
  let UnifiedType { unified, specifics } =
    syn::parse::<UnifiedType>(toks).unwrap();
  let mut specifics = specifics
    .into_iter()
    .map(|x| x.path.to_token_stream())
    .collect::<Vec<proc_macro2::TokenStream>>();
  specifics.push(quote! {
    aurum::core::RegistryMsg<#unified>
  });
  let variants = std::iter::repeat(('A'..='B').collect::<Vec<_>>())
    .take((specifics.len() as f64).log(1.9).ceil() as usize)
    .multi_cartesian_product()
    .take(specifics.len())
    .map(|x| {
      Ident::new(
        x.into_iter().collect::<String>().as_str(),
        Span::call_site(),
      )
    })
    .collect::<Vec<_>>();
  let code = TokenStream::from(quote! {
    #[derive(
      serde::Serialize, serde::Deserialize, std::cmp::Eq,
      std::cmp::PartialEq, std::hash::Hash, std::clone::Clone, 
      std::marker::Copy, std::cmp::PartialOrd, std::cmp::Ord
    )]
    enum #unified {
      #(#variants,)*
    }
    impl std::fmt::Display for #unified {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let specific = match self {
          #(
            #unified::#variants => std::any::type_name::<#specifics>(),
          )*
        };
        write!(f, "{}<{}>", std::any::type_name::<#unified>(), specific)
      }
    }
    impl std::fmt::Debug for #unified {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Display>::fmt(self, f)
      }
    }
    #(
      impl aurum::core::Case<#specifics> for #unified {
        const VARIANT: #unified = #unified::#variants;
      }
    )*
  });
  super::write_and_fmt(unified.to_string(), &code).expect("can't save codegen");
  code
}

struct UnifiedType {
  unified: Ident,
  specifics: Vec<SpecificType>,
}
impl Parse for UnifiedType {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let unified = input.parse::<Ident>()?;
    input.parse::<Token![=]>()?;
    let specifics =
      Punctuated::<SpecificType, Token![|]>::parse_terminated(&input)?
        .into_iter()
        .collect::<Vec<SpecificType>>();
    Ok(UnifiedType {
      unified: unified,
      specifics: specifics,
    })
  }
}

struct SpecificType {
  path: TypePath,
}
impl Parse for SpecificType {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let path = input.parse::<TypePath>()?;
    Ok(SpecificType { path: path })
  }
}
