extern crate proc_macro;
use indoc::formatdoc;
use proc_macro::TokenStream;
use syn;

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    let type_id = ast.ident.to_string();
    let data_enum: syn:: DataEnum = match ast.data {
      syn::Data::Enum(x) => x,
      _ => panic!("Only enums are supported")
    };
    let variants: Vec<&syn::Variant> = data_enum.variants.iter().collect();
    let translates = variants.iter().filter_map(|x| aurum_tagged(*x))
      .collect::<Vec<AurumVariant>>();
    let impls = translates.iter().map(|av: &AurumVariant| {
      let f = formatdoc! {"
        impl std::convert::From<{from}> for {id} {{
          fn from(item: {from}) -> {id} {{
            {id}::{variant}(item)
          }}
        }}
        ",
        id = type_id,
        from = av.field,
        variant = av.variant
      };
      f.to_string()
    }).collect::<Vec<String>>();

    let code = impls.join("\n");
    println!("Generated code for {}: \n\n{}", type_id, code);

    code.parse().unwrap()
}

#[derive(Debug)]
struct AurumVariant {
  variant: String,
  field: String
}

fn aurum_tagged(variant: &syn::Variant) -> Option<AurumVariant> {
  let err = "Aurum translations must contain a single, unnamed field";
  let id = variant.ident.to_string();
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
  let type_str = match fields.remove(0) {
    syn::Type::Path(p) => path_to_string(&p.path),
    _ => panic!("Type is not a type path")
  };
  Some(AurumVariant { variant: id, field: type_str } )
}

fn path_to_string(path: &syn::Path) -> String {
  path.segments.iter().map(|seg| seg.ident.to_string())
    .collect::<Vec<String>>().join("::")
}