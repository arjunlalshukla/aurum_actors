extern crate proc_macro;
use proc_macro::TokenStream;
use syn;

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
    println!("item: \"{}\"", item.to_string());

    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    let type_id: syn:: Ident = ast.ident;
    let data_enum: syn:: DataEnum = match ast.data {
      syn::Data::Enum(x) => x,
      _ => panic!("Only enums are supported")
    };
    let variants: Vec<&syn::Variant> = data_enum.variants.iter().collect();
    let translates = variants.iter().filter_map(|x| aurum_tagged(*x))
      .collect::<Vec<AurumVariant>>();
    println!("identifier = {}", type_id.to_string());
    for t in translates {
      println!("aurum translatable: {:?}", t);
    }
    for v in variants {
      aurum_tagged(v);
    }

    TokenStream::new()
}

#[derive(Debug)]
struct AurumVariant {
  variant: String,
  field: String
}

fn aurum_tagged(variant: &syn::Variant) -> Option<AurumVariant> {
  let tag = "aurum".to_string();
  let err = "Aurum translations must contain a single, unnamed field";
  let id = variant.ident.to_string();
  let tagged = variant.attrs.iter().map(|attr| path_to_string(&attr.path))
    .any(|s| s == tag);
  if !tagged {
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