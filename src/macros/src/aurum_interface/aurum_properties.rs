use std::convert::TryFrom;
use syn::{
  parenthesized, parse::Parse, parse::ParseStream, punctuated::Punctuated,
  Attribute, Expr, ExprPath, Token,
};
pub struct AurumProperties {
  pub non_local: bool,
  //priority: Option<Expr>
}
impl Default for AurumProperties {
  fn default() -> AurumProperties {
    AurumProperties { non_local: true }
  }
}
impl TryFrom<Vec<Attribute>> for AurumProperties {
  type Error = ();
  fn try_from(attrs: Vec<Attribute>) -> Result<Self, Self::Error> {
    let mut i = attrs.into_iter().filter(|attr| attr.path.is_ident("aurum"));
    match (i.next(), i.next().is_some()) {
      (None, false) => Err(()),
      (Some(attr), false) => syn::parse(attr.tokens.into()).map_err(|_| ()),
      (Some(_), true) => panic!("Only use exactly one aurum annotation"),
      (None, true) => panic!("WTF?! this should never happen..."),
    }
  }
}
impl Parse for AurumProperties {
  fn parse(input: ParseStream) -> syn::Result<AurumProperties> {
    let mut props = AurumProperties::default();
    if input.is_empty() {
      return Ok(props);
    }

    let inner;
    parenthesized!(inner in input);
    let args = Punctuated::<Expr, Token![,]>::parse_terminated(&inner)?
      .into_iter()
      .collect::<Vec<Expr>>();

    for attr in args {
      match attr {
        Expr::Path(ExprPath {
          attrs: _,
          qself: _,
          path: p,
        }) => {
          let mut i = p.segments.into_iter();
          let flag = i.next().unwrap();
          if i.next().is_some() {
            panic!("Aurum flags are singular identifiers");
          }
          match flag.arguments {
            syn::PathArguments::None => (),
            _ => panic!("Aurum flags should not contain generics"),
          }
          match flag.ident.to_string().as_str() {
            "local" if props.non_local => props.non_local = false,
            "local" => already_defined("local"),
            _ => (),
          }
        }
        // This is for priorities later
        //Expr::Assign(a) => (),
        _ => panic!("Only identifiers are accepted in Aurum arguments"),
      }
    }

    Ok(props)
  }
}

fn already_defined(s: &str) {
  panic!("Do not define Aurum flag {} more than once", s)
}
