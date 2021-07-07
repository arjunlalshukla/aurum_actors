use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{self, Span};
use quote::{quote, ToTokens};
use syn::parse::{Error, Parse};
use syn::{braced, punctuated::Punctuated, Ident, Token, Type, Visibility};

pub fn unify_impl(toks: TokenStream) -> TokenStream {
  let Unification {
    unified_name: unified,
    unified_vis: vis,
    root_types: specifics,
    interfaces,
  } = match syn::parse(toks) {
    Ok(x) => x,
    Err(e) => return e.to_compile_error().into()
  };

  let specifics = vec![
    quote!(aurum_actors::core::RegistryMsg<#unified>),
    quote!(aurum_actors::cluster::ClusterMsg<#unified>),
    quote!(aurum_actors::cluster::HeartbeatReceiverMsg),
    quote!(aurum_actors::cluster::devices::DeviceServerMsg),
    quote!(aurum_actors::cluster::devices::DeviceClientMsg<#unified>),
    quote!(aurum_actors::cluster::devices::HBReqSenderMsg),
    quote!(aurum_actors::cluster::crdt::CausalMsg<aurum_actors::cluster::devices::Devices>),
    quote!(aurum_actors::testkit::LoggerMsg)
  ]
  .into_iter()
  .chain(specifics.into_iter().map(|x| x.to_token_stream()))
  .collect::<Vec<proc_macro2::TokenStream>>();

  let interfaces = vec![
    quote!(aurum_actors::cluster::IntraClusterMsg<#unified>),
    quote!(aurum_actors::cluster::devices::DeviceServerRemoteMsg),
    quote!(aurum_actors::cluster::devices::DeviceClientRemoteMsg<#unified>),
    quote!(aurum_actors::cluster::crdt::CausalIntraMsg<aurum_actors::cluster::devices::Devices>),
    quote!(aurum_actors::cluster::devices::HBReqSenderRemoteMsg),
  ]
  .into_iter()
  .chain(interfaces.into_iter().map(|x| x.to_token_stream()))
  .collect::<Vec<proc_macro2::TokenStream>>();

  let mut all = specifics.clone();
  all.append(&mut interfaces.clone());
  let all_variants = std::iter::repeat(('A'..='B').collect_vec())
    .take((all.len() as f64).log(1.9).ceil() as usize)
    .multi_cartesian_product()
    .take(all.len())
    .map(|x| Ident::new(x.into_iter().collect::<String>().as_str(), Span::call_site()))
    .collect_vec();
  let specific_variants = all_variants.clone().into_iter().take(specifics.len()).collect_vec();
  let code = TokenStream::from(quote! {
    #[derive(
      serde::Serialize, serde::Deserialize, std::cmp::Eq,
      std::cmp::PartialEq, std::hash::Hash, std::clone::Clone,
      std::marker::Copy, std::cmp::PartialOrd, std::cmp::Ord
    )]
    #vis enum #unified {
      #(#all_variants,)*
    }
    impl aurum_actors::core::UnifiedType for #unified {
      fn has_interface(self, interface: Self) -> bool {
        match self {
          #(
            #unified::#specific_variants => <#specifics as aurum_actors::core::RootMessage<#unified>>::has_interface(interface),
          )*
          _ => false
        }
      }
    }
    impl std::fmt::Display for #unified {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let specific = match self {
          #(
            #unified::#all_variants => std::any::type_name::<#all>(),
          )*
        };
        write!(f, "{}<{}>", std::any::type_name::<#unified>(), specific)
      }
    }
    impl std::fmt::Debug for #unified {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Display>::fmt(self, f)
      }
    }
    #(
      impl aurum_actors::core::Case<#all> for #unified {
        const VARIANT: #unified = #unified::#all_variants;
      }
    )*
  });
  code
}

struct Unification {
  unified_name: Ident,
  unified_vis: Visibility,
  root_types: Vec<Type>,
  interfaces: Vec<Type>,
}
impl Parse for Unification {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let mut unified_name: Option<(Visibility, Ident)> = None;
    let mut root_types: Option<Vec<Type>> = None;
    let mut interfaces: Option<Vec<Type>> = None;

    while !input.is_empty() {
      let key = input.parse::<Ident>()?;
      input.parse::<Token![=]>()?;
      let key_str = key.to_string();
      match key_str.as_str() {
        "unified_name" => {
          if unified_name.is_some() {
            return Err(Error::new(key.span(), "Duplicate key `unified_name`"));
          }
          unified_name = Some((input.parse::<Visibility>()?, input.parse::<Ident>()?));
        }
        "root_types" => {
          if root_types.is_some() {
            return Err(Error::new(key.span(), "Duplicate key `root_types`"));
          }
          let content;
          braced!(content in input);
          root_types = Some(
            Punctuated::<Type, Token![,]>::parse_terminated(&content)?.into_iter().collect()
          );
        }
        "interfaces" => {
          if interfaces.is_some() {
            return Err(Error::new(key.span(), "Duplicate key `interfaces`"));
          }
          let content;
          braced!(content in input);
          interfaces = Some(
            Punctuated::<Type, Token![,]>::parse_terminated(&content)?.into_iter().collect()
          );
        }
        _ => return Err(Error::new(key.span(), format!("Unrecognized key `{}`", key_str)))
      }
      input.parse::<Token![;]>()?;
    }

    let (vis, name) = match unified_name {
      Some(x) => x,
      None => return Err(Error::new(input.span(), "Missing required key `unified_name`"))
    };

    Ok(Self {
      unified_name: name,
      unified_vis: vis,
      root_types: root_types.unwrap_or(vec![]),
      interfaces: interfaces.unwrap_or(vec![])
    })
  }
}