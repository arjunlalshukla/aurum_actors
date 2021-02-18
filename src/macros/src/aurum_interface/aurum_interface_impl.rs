use proc_macro::TokenStream;
use proc_macro2;
use quote::{quote, ToTokens};
use std::convert::TryFrom;
use syn::{DeriveInput, Ident, TypePath};

use super::{AurumProperties, AurumVariant};
use crate::write_and_fmt;

pub fn aurum_interface_impl(ast: DeriveInput) -> TokenStream {
  let type_tokens = ast.ident.to_string().parse().unwrap();
  let generics = ast.generics.params;
  let generic_types = generics
    .iter()
    .map(|x| match x {
      syn::GenericParam::Type(t) => &t.ident,
      _ => panic!("Only generic types are allow, no consts or lifetimes"),
    })
    .collect::<Vec<_>>();
  let where_clause = ast.generics.where_clause;
  let type_id = syn::parse_macro_input!(type_tokens as TypePath);
  let type_id_with_generics: proc_macro2::TokenStream = quote! {
    #type_id<#(#generic_types),*>
  };
  let type_props = AurumProperties::try_from(ast.attrs).unwrap_or_default();
  let data_enum: syn::DataEnum = match ast.data {
    syn::Data::Enum(x) => x,
    _ => panic!("Only enums are supported"),
  };
  let translates = data_enum
    .variants
    .into_iter()
    .filter_map(|x| AurumVariant::try_from(x).ok())
    .collect::<Vec<AurumVariant>>();
  let interfaces = translates
    .iter()
    .map(|x| x.field.to_token_stream().into())
    .collect::<Vec<proc_macro2::TokenStream>>();
  let variants = translates
    .iter()
    .map(|x| &x.variant)
    .collect::<Vec<&Ident>>();
  let mut non_locals = translates
    .iter()
    .filter(|x| x.props.non_local)
    .map(|x| x.field.to_token_stream().into())
    .collect::<Vec<proc_macro2::TokenStream>>();
  let mut cases = non_locals.clone();
  if type_props.non_local {
    non_locals.push(type_id_with_generics.clone());
  }
  cases.push(type_id_with_generics.clone());

  let code = TokenStream::from(quote! {
    #(
      impl<#generics> std::convert::From<#interfaces>
      for #type_id_with_generics {
        fn from(item: #interfaces) -> #type_id_with_generics {
          #type_id::#variants(item)
        }
      }
    )*

    #(
      impl<#generics> aurum::core::HasInterface<#non_locals>
      for #type_id_with_generics {}
    )*

    impl<__Unified, #generics> aurum::core::SpecificInterface<__Unified>
    for #type_id_with_generics
    where __Unified: aurum::core::UnifiedBounds #(+ aurum::core::Case<#cases>)* ,
    #where_clause
    {
      fn deserialize_as(item: __Unified, bytes: Vec<u8>) ->
       std::result::Result<Self, aurum::core::DeserializeError<__Unified>> {
        #(
          if <__Unified as aurum::core::Case<#non_locals>>::VARIANT == item {
            return aurum::core::deserialize
              ::<__Unified, #type_id_with_generics, #non_locals>(item, bytes)
          }
        )*
        return std::result::Result::Err(
          aurum::core::DeserializeError::IncompatibleInterface(item)
        );
      }
    }
  });
  write_and_fmt(ast.ident.to_string(), &code).expect("can't save codegen");
  code
}
