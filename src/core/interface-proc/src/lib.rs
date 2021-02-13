extern crate proc_macro;
use quote::quote;
use proc_macro::TokenStream;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use syn::{self, Ident, TypePath};

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    let type_tokens = ast.ident.to_string().parse().unwrap();
    let type_id = syn::parse_macro_input!(type_tokens as TypePath); 
    let type_remote = ast.attrs.iter().any(|attr| attr.path.is_ident("aurum"));
    let data_enum: syn:: DataEnum = match ast.data {
      syn::Data::Enum(x) => x,
      _ => panic!("Only enums are supported")
    };
    let translates = data_enum.variants.iter().filter_map(|x| aurum_tagged(x))
      .collect::<Vec<AurumVariant>>();
    let interfaces = translates.iter().map(|x| x.field).collect::<Vec<&TypePath>>();
    let variants = translates.iter().map(|x| x.variant).collect::<Vec<&Ident>>();
    let mut non_locals = translates.iter().filter(|x| x.non_local)
      .map(|x| x.field)
      .collect::<Vec<&TypePath>>();
    if type_remote {
      non_locals.push(&type_id);
    }

    let code = TokenStream::from(quote! {
      #(
        impl std::convert::From<#interfaces> for #type_id {
          fn from(item: #interfaces) -> #type_id {
            #type_id::#variants(item)
          }
        }
      )*

      #(
        impl aurum::core::HasInterface<#non_locals> for #type_id {}
      )*

      impl<Unified> aurum::core::SpecificInterface<Unified> for #type_id
       where Unified: std::cmp::Eq + std::fmt::Debug #(+ aurum::core::Case<#non_locals>)* {
        fn deserialize_as(item: Unified, bytes: Vec<u8>) ->
         std::result::Result<Self, aurum::core::DeserializeError<Unified>> {
          #(
            if <Unified as aurum::core::Case<#non_locals>>::VARIANT == item {
              return aurum::core::deserialize::<Unified, #type_id, #non_locals>(item, bytes)
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

struct AurumVariant<'a> {
  variant: &'a Ident,
  field: &'a TypePath,
  non_local: bool
}

fn aurum_tagged(variant: &syn::Variant) -> Option<AurumVariant> {
  let err = "Aurum translations must contain a single, unnamed field";
  if !variant.attrs.iter().any(|attr| attr.path.is_ident("aurum")) {
    return None;
  }
  let mut fields = match &variant.fields {
    syn::Fields::Unnamed(u) => u.unnamed.iter().map(|x| &x.ty)
      .collect::<Vec<&syn::Type>>(),
    _ => panic!(err)
  };
  if fields.len() != 1 {
    panic!(err);
  }
  let type_path = match fields.remove(0) {
    syn::Type::Path(p) => p,
    _ => panic!("Type is not a type path")
  };
  Some(AurumVariant { variant: &variant.ident,  field: type_path, non_local: true} )
}

fn write_and_fmt<S: ToString>(file: String, code: S) -> io::Result<()> {
  fs::create_dir_all("./code-gen/")?;
  let p = Path::new("./code-gen").join(format!("{}.rs", file));
  fs::write(p.clone(), code.to_string())?;
  Command::new("rustfmt")
      .arg(p)
      .spawn()?
      .wait()?;
  Ok(())
}