use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseBuffer, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::{
  parenthesized, Attribute, DeriveInput, Error, Expr, ExprPath, Generics, Ident, Token, Type,
  Variant,
};

pub struct RootImpl {
  type_id: Ident,
  generics: Generics,
  generic_ids: Vec<Ident>,
  type_attr: AurumProperties,
  variants: Vec<AurumVariant>,
}
impl RootImpl {
  pub fn derive(ast: DeriveInput) -> syn::Result<Self> {
    derive(ast)
  }
  pub fn expand(&self) -> TokenStream {
    expand(self)
  }
}

fn derive(ast: DeriveInput) -> syn::Result<RootImpl> {
  let aspan = ast.span();
  let generics = ast.generics;
  let mut generic_ids = Vec::new();
  for param in generics.params.clone() {
    match param {
      syn::GenericParam::Type(t) => generic_ids.push(t.ident),
      syn::GenericParam::Lifetime(l) => {
        return Err(Error::new(
          l.lifetime.span(),
          "AurumInterface does not allow lifetime parameters.",
        ))
      }
      syn::GenericParam::Const(c) => {
        return Err(Error::new(
          c.const_token.span,
          "AurumInterface does not allow const parameters.",
        ))
      }
    }
  }

  let data_enum: syn::DataEnum = match ast.data {
    syn::Data::Enum(x) => x,
    _ => return Err(Error::new(aspan, "AurumInterface only supports enums.")),
  };
  let mut variants = vec![];
  for v in data_enum.variants.into_iter().map(AurumVariant::get) {
    variants.push(v?);
  }

  Ok(RootImpl {
    type_id: ast.ident,
    generics: generics,
    generic_ids: generic_ids,
    type_attr: AurumProperties::get(ast.attrs)?.unwrap_or_default(),
    variants: variants,
  })
}

pub struct AurumVariant {
  pub variant_name: Ident,
  pub field_types: Vec<Type>,
  pub field_names: Option<Vec<Ident>>,
  pub props: Option<AurumProperties>,
}
impl AurumVariant {
  fn get(variant: Variant) -> Result<Self, Error> {
    let vspan = variant.fields.span();
    let props = AurumProperties::get(variant.attrs)?;
    let (types, names) = match variant.fields {
      syn::Fields::Named(n) => n.named.into_iter().map(|x| (x.ty, x.ident)).unzip(),
      syn::Fields::Unnamed(u) => u.unnamed.into_iter().map(|x| (x.ty, x.ident)).unzip(),
      syn::Fields::Unit => (vec![], vec![]),
    };

    if types.len() != 1 && props.is_some() {
      return Err(Error::new(vspan, "AurumInterface variants must have exactly one field."));
    }

    Ok(AurumVariant {
      variant_name: variant.ident,
      field_types: types,
      field_names: names.into_iter().collect(),
      props: props,
    })
  }
}

pub struct AurumProperties {
  pub local: bool,
}
impl Default for AurumProperties {
  fn default() -> AurumProperties {
    AurumProperties {
      local: false,
    }
  }
}
impl AurumProperties {
  fn get(attrs: Vec<Attribute>) -> Result<Option<Self>, Error> {
    let mut i = attrs.into_iter().filter(|attr| attr.path.is_ident("aurum"));
    match (i.next(), i.next()) {
      (None, None) => Ok(None),
      (Some(attr), None) => Ok(Some(syn::parse(attr.tokens.into())?)),
      (Some(_), Some(attr)) => Err(Error::new(
        attr.path.span(),
        "AurumInterface does not allow more than one 'aurum' annotation in the same place.",
      )),
      _ => unreachable!(),
    }
  }
}

// We need this function because parenthesized! is lame...
fn my_parenthesized<'a>(input: &'a ParseBuffer<'a>) -> syn::Result<(ParseBuffer<'a>, Paren)> {
  let inner;
  let paren = parenthesized!(inner in input);
  Ok((inner, paren))
}

impl Parse for AurumProperties {
  fn parse(input: ParseStream) -> syn::Result<AurumProperties> {
    let default = AurumProperties::default();
    if input.is_empty() {
      return Ok(default);
    }

    let (inner, _) = my_parenthesized(&input).map_err(|_| {
      Error::new(input.span(), "You must have parentheses around arguments to 'aurum'")
    })?;
    let args =
      Punctuated::<Expr, Token![,]>::parse_terminated(&inner)?.into_iter().collect::<Vec<Expr>>();

    let mut local: Option<bool> = None;

    for attr in args {
      match attr {
        Expr::Path(ExprPath {
          attrs: _,
          qself: _,
          path: p,
        }) => {
          let flag = match p.get_ident().cloned() {
            Some(i) => i,
            None => return Err(Error::new(p.span(), "'aurum' flags are singular identifiers.")),
          };
          match flag.to_string().as_str() {
            "local" => {
              if local.is_some() {
                return Err(Error::new(flag.span(), "Duplicate flag `local`"));
              }
              local = Some(true);
            }
            _ => return Err(Error::new(flag.span(), format!("Unknown flag `{}`", flag))),
          }
        }
        _ => {
          return Err(Error::new(
            attr.span(),
            "'aurum' attribute only accepts identifiers as arguments",
          ))
        }
      }
    }

    Ok(AurumProperties {
      local: local.unwrap_or(default.local),
    })
  }
}

fn expand(root: &RootImpl) -> TokenStream {
  let type_id = &root.type_id;
  let generic_ids = &root.generic_ids;
  let generic_params = &root.generics.params;
  let type_id_with_generics: proc_macro2::TokenStream = quote!(#type_id<#(#generic_ids),*>);
  let where_predicates = root.generics.where_clause.as_ref().map(|x| &x.predicates);

  let from_impls = root.variants.iter().filter(|v| v.props.is_some()).map(|variant| {
    let variant_name = &variant.variant_name;
    let field_type = &variant.field_types[0];
    let convert_toks = match variant.field_names.as_ref().map(|v| &v[0]) {
      Some(name) => quote!(#type_id::#variant_name { #name: item }),
      None => quote!(#type_id::#variant_name(item)),
    };
    quote! {
      impl<#generic_params> ::std::convert::From<#field_type> for #type_id_with_generics
      where
        #where_predicates
      {
        fn from(item: #field_type) -> #type_id_with_generics {
          #convert_toks
        }
      }
    }
  });

  let mut non_locals = root
    .variants
    .iter()
    .filter(|x| x.props.as_ref().filter(|x| !x.local).is_some())
    .map(|x| x.field_types[0].to_token_stream().into())
    .collect::<Vec<proc_macro2::TokenStream>>();
  let mut cases = non_locals.clone();
  if !root.type_attr.local {
    non_locals.push(type_id_with_generics.clone());
  }
  cases.push(type_id_with_generics.clone());

  let code = TokenStream::from(quote! {
    #(#from_impls)*

    impl<__Unified, #generic_params> aurum_actors::core::RootMessage<__Unified>
    for #type_id_with_generics
    where __Unified: aurum_actors::core::UnifiedType #(+ aurum_actors::core::Case<#cases>)* ,
    #where_predicates
    {
      fn deserialize_as(
        interface: __Unified,
        intp: aurum_actors::core::Interpretations,
        bytes: &[u8]
      ) -> ::std::result::Result<
        aurum_actors::core::LocalActorMsg<Self>,
        aurum_actors::core::DeserializeError<__Unified>
      > {
        #(
          if <__Unified as aurum_actors::core::Case<#non_locals>>::VARIANT
            == interface {
            return aurum_actors::core::deserialize_msg
              ::<__Unified, #type_id_with_generics, #non_locals>(
                interface,
                intp,
                bytes
              )
          }
        )*
        return ::std::result::Result::Err(
          aurum_actors::core::DeserializeError::IncompatibleInterface(
            interface,
            <__Unified as aurum_actors::core::Case<#type_id_with_generics>>::VARIANT
          )
        );
      }

      fn has_interface(interface: __Unified) -> bool {
        #(<__Unified as aurum_actors::core::Case<#non_locals>>::VARIANT == interface || )* false
      }
    }
  });
  code
}
