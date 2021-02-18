use std::convert::TryFrom;
use syn::{Ident, TypePath, Variant};

use super::AurumProperties;

pub struct AurumVariant {
  pub variant: Ident,
  pub field: TypePath,
  pub props: AurumProperties,
}
impl TryFrom<Variant> for AurumVariant {
  type Error = ();

  fn try_from(variant: Variant) -> Result<Self, Self::Error> {
    let err = "Aurum translations must contain a single, unnamed field";
    let props = AurumProperties::try_from(variant.attrs)?;
    let type_path = {
      let mut fields = match variant.fields {
        syn::Fields::Unnamed(u) => u
          .unnamed
          .into_iter()
          .map(|x| x.ty)
          .collect::<Vec<syn::Type>>(),
        _ => panic!(err),
      };
      if fields.len() != 1 {
        panic!(err);
      }
      match fields.remove(0) {
        syn::Type::Path(p) => p,
        _ => panic!("Type is not a type path"),
      }
    };
    Ok(AurumVariant {
      variant: variant.ident,
      field: type_path,
      props: props, //aurum_properties(aurum_toks).expect("Failed to parse aurum attribute")
    })
  }
}
