extern crate proc_macro;
use quote::{ToTokens, quote};
use proc_macro::TokenStream;
use proc_macro2;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use syn::{self, Expr, ExprPath, Ident, Result, Token, TypePath, parenthesized, 
  parse::Parse, parse::ParseStream, punctuated::Punctuated};

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    let type_tokens = ast.ident.to_string().parse().unwrap();
    let generics = ast.generics.params;
    let generic_types = generics.iter().map(|x| match x {
      syn::GenericParam::Type(t) => &t.ident,
      _ => panic!("Only generic types are allow, no consts or lifetimes")
    }).collect::<Vec<_>>();
    let where_clause = ast.generics.where_clause;
    let type_id = syn::parse_macro_input!(type_tokens as TypePath); 
    let type_id_with_generics: proc_macro2::TokenStream = quote! {
      #type_id<#(#generic_types),*>
    };
    let type_props = aurum_tokens(ast.attrs)
      .map(|x| syn::parse::<AurumProperties>(x).unwrap()).unwrap_or_default();
    let data_enum: syn:: DataEnum = match ast.data {
      syn::Data::Enum(x) => x,
      _ => panic!("Only enums are supported")
    };
    let translates = data_enum.variants.into_iter()
      .filter_map(|x| aurum_tagged(x)).collect::<Vec<AurumVariant>>();
    let interfaces = translates.iter()
      .map(|x| x.field.to_token_stream().into())
      .collect::<Vec<proc_macro2::TokenStream>>();
    let variants = translates.iter().map(|x| &x.variant).collect::<Vec<&Ident>>();
    let mut non_locals = translates.iter().filter(|x| x.props.non_local)
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

struct AurumVariant {
  variant: Ident,
  field: TypePath,
  props: AurumProperties
}

struct AurumProperties {
  non_local: bool,
  //priority: Option<Expr>
}
impl Default for AurumProperties {
  fn default() -> AurumProperties {
    AurumProperties {
      non_local: true
    }
  }
}
impl Parse for AurumProperties {
  fn parse(input: ParseStream) -> Result<AurumProperties> {
    let mut props = AurumProperties::default();
    if input.is_empty() {
      return Ok(props);
    }

    let inner;
    parenthesized!(inner in input);
    let args = Punctuated::<Expr, Token![,]>::parse_terminated(&inner)?
      .into_iter().collect::<Vec<Expr>>();
  
    for attr in args {
      match attr {
        Expr::Path(ExprPath {attrs: _, qself: _, path: p}) => {
          let mut i = p.segments.into_iter();
          let flag = i.next().unwrap();
          if i.next().is_some() {
            panic!("Aurum flags are singular identifiers");
          }
          match flag.arguments {
            syn::PathArguments::None => (),
            _ => panic!("Aurum flags should not contain generics")
          }
          match flag.ident.to_string().as_str() {
            "local" if props.non_local => props.non_local = false,
            "local" => already_defined("local"),
            _ => ()
          }
        }
        // This is for priorities later
        //Expr::Assign(a) => (),
        _ => panic!("Only identifiers are accepted in Aurum arguments")
      }
    }
  
    Ok(props)
  }
}

fn aurum_tagged(variant: syn::Variant) -> Option<AurumVariant> {
  let err = "Aurum translations must contain a single, unnamed field";
  let aurum_toks = TokenStream::from(aurum_tokens(variant.attrs)?);
  let type_path = {
    let mut fields = match variant.fields {
      syn::Fields::Unnamed(u) => u.unnamed.into_iter().map(|x| x.ty)
        .collect::<Vec<syn::Type>>(),
      _ => panic!(err)
    };
    if fields.len() != 1 {
      panic!(err);
    }
    match fields.remove(0) {
      syn::Type::Path(p) => p,
      _ => panic!("Type is not a type path")
    }
  };
  Some(AurumVariant { 
    variant: variant.ident,  
    field: type_path, 
    props: syn::parse(aurum_toks).unwrap()
    //aurum_properties(aurum_toks).expect("Failed to parse aurum attribute")
  } )
}

fn aurum_tokens(attrs: Vec<syn::Attribute>) -> Option<TokenStream> {
  let mut i = attrs.into_iter()
    .filter(|attr| attr.path.is_ident("aurum"));
  match (i.next(), i.next().is_some()) {
    (None, false) => None,
    (Some(attr), false) => Some(attr.tokens.into()),
    (Some(_), true) => panic!("Only use exactly one aurum annotation"),
    (None, true) => panic!("WTF?! this should never happen...")
  }
}

fn already_defined(s: &str) {
  panic!(format!("Do not define Aurum flag {} more than once", s))
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