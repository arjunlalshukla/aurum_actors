use proc_macro::TokenStream;
use proc_macro2;
use quote::{quote, ToTokens};
use std::convert::TryFrom;
use syn::{DeriveInput, TypePath};

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

  let from_impls = translates.iter().map(|variant| {
    let variant_name = &variant.variant_name;
    let field_type = &variant.field_type;
    match &variant.field_name {
      Some(name) => quote! {
        impl<#generics> std::convert::From<#field_type>
        for #type_id_with_generics {
          fn from(item: #field_type) -> #type_id_with_generics {
            #type_id::#variant_name { #name: item }
          }
        }
      },
      None => quote! {
        impl<#generics> std::convert::From<#field_type>
        for #type_id_with_generics {
          fn from(item: #field_type) -> #type_id_with_generics {
            #type_id::#variant_name(item)
          }
        }
      },
    }
  });

  let mut non_locals = translates
    .iter()
    .filter(|x| x.props.non_local)
    .map(|x| x.field_type.to_token_stream().into())
    .collect::<Vec<proc_macro2::TokenStream>>();
  let mut cases = non_locals.clone();
  if type_props.non_local {
    non_locals.push(type_id_with_generics.clone());
  }
  cases.push(type_id_with_generics.clone());

  let code = TokenStream::from(quote! {
    #(#from_impls)*

    impl<__Unified, #generics> aurum::core::RootMessage<__Unified>
    for #type_id_with_generics
    where __Unified: aurum::core::UnifiedType #(+ aurum::core::Case<#cases>)* ,
    #where_clause
    {
      fn deserialize_as(
        interface: __Unified, 
        intp: aurum::core::Interpretations, 
        bytes: &[u8]
      ) -> std::result::Result<
        aurum::core::LocalActorMsg<Self>, 
        aurum::core::DeserializeError<__Unified>
      > {
        #(
          if <__Unified as aurum::core::Case<#non_locals>>::VARIANT 
            == interface {
            return aurum::core::deserialize_msg
              ::<__Unified, #type_id_with_generics, #non_locals>(
                interface, 
                intp,
                bytes
              )
          }
        )*
        return std::result::Result::Err(
          aurum::core::DeserializeError::IncompatibleInterface(interface, <__Unified as aurum::core::Case<#type_id_with_generics>>::VARIANT )
        );
      }

      fn has_interface(interface: __Unified) -> bool {
        #(<__Unified as aurum::core::Case<#non_locals>>::VARIANT == interface || )* false
      }
    }
  });
  write_and_fmt(ast.ident.to_string(), &code).expect("can't save codegen");
  code
}
