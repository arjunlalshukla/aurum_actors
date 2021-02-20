use itertools::Itertools;
use std::convert::TryFrom;
use syn::{Ident, TypePath, Variant};

use super::AurumProperties;

pub struct AurumVariant {
  pub variant_name: Ident,
  pub field_type: TypePath,
  pub field_name: Option<Ident>,
  pub props: AurumProperties,
}
impl TryFrom<Variant> for AurumVariant {
  type Error = ();

  fn try_from(variant: Variant) -> Result<Self, Self::Error> {
    let props = AurumProperties::try_from(variant.attrs)?;
    let fields: Vec<(syn::Type, Option<Ident>)> = match variant.fields {
      syn::Fields::Named(n) => n
        .named
        .into_iter()
        .map(|x| (x.ty, x.ident))
        .collect(),
      syn::Fields::Unnamed(u) => u
        .unnamed
        .into_iter()
        .map(|x| (x.ty, x.ident))
        .collect(),
      syn::Fields::Unit => Vec::new(),
    };
    match fields.into_iter().exactly_one() {
      Ok((syn::Type::Path(type_path), name)) => Ok(AurumVariant {
        variant_name: variant.ident,
        field_type: type_path,
        field_name: name,
        props: props, 
      }),
      Ok((_, _)) => panic!("Type is not a type path"),
      Err(_) => panic!("Aurum variants contain exactly one field")
    }
  }
}
