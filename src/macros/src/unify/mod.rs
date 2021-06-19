use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{self, Span};
use quote::{quote, ToTokens};
use syn::parse::Parse;
use syn::{punctuated::Punctuated, Ident, Token, TypePath, Visibility};

pub fn unify_impl(toks: TokenStream) -> TokenStream {
  let UnifiedType { unified, specifics, interfaces, vis } =
    syn::parse::<UnifiedType>(toks).unwrap();
  let mut specifics = specifics
    .into_iter()
    .map(|x| x.path.to_token_stream())
    .collect::<Vec<proc_macro2::TokenStream>>();
  specifics.push(quote! {
    aurum::core::RegistryMsg<#unified>
  });
  specifics.push(quote! {
    aurum::cluster::ClusterMsg<#unified>
  });
  specifics.push(quote! {
    aurum::cluster::HeartbeatReceiverMsg
  });
  specifics.push(quote! {
    aurum::cluster::devices::DeviceServerMsg
  });
  specifics.push(quote! {
    aurum::cluster::devices::DeviceClientMsg<#unified>
  });
  specifics.push(quote! {
    aurum::cluster::devices::HBReqSenderMsg
  });
  specifics.push(quote! {
    aurum::cluster::crdt::CausalMsg<aurum::cluster::devices::Devices>
  });
  specifics.push(quote! {
    aurum::testkit::LoggerMsg
  });
  let mut interfaces = interfaces
    .into_iter()
    .map(|x| x.path.to_token_stream())
    .collect::<Vec<proc_macro2::TokenStream>>();
  interfaces.push(quote! {
    aurum::cluster::IntraClusterMsg<#unified>
  });
  interfaces.push(quote! {
    aurum::cluster::devices::DeviceServerRemoteMsg
  });
  interfaces.push(quote! {
    aurum::cluster::devices::DeviceClientRemoteMsg<#unified>
  });
  interfaces.push(quote! {
    aurum::cluster::crdt::CausalIntraMsg<aurum::cluster::devices::Devices>
  });
  interfaces.push(quote! {
    aurum::cluster::devices::HBReqSenderRemoteMsg
  });
  let mut all = specifics.clone();
  all.append(&mut interfaces.clone());
  let all_variants = std::iter::repeat(('A'..='B').collect_vec())
    .take((all.len() as f64).log(1.9).ceil() as usize)
    .multi_cartesian_product()
    .take(all.len())
    .map(|x| {
      Ident::new(
        x.into_iter().collect::<String>().as_str(),
        Span::call_site(),
      )
    })
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
    impl aurum::core::UnifiedType for #unified {
      fn has_interface(self, interface: Self) -> bool {
        match self {
          #(
            #unified::#specific_variants => <#specifics as aurum::core::RootMessage<#unified>>::has_interface(interface),
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
      impl aurum::core::Case<#all> for #unified {
        const VARIANT: #unified = #unified::#all_variants;
      }
    )*
  });
  super::write_and_fmt(unified.to_string(), &code).expect("can't save codegen");
  code
}

struct UnifiedType {
  unified: Ident,
  specifics: Vec<SpecificType>,
  interfaces: Vec<SpecificType>,
  vis: Visibility
}
impl Parse for UnifiedType {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let vis = input.parse::<Visibility>()?;
    let unified = input.parse::<Ident>()?;
    input.parse::<Token![=]>()?;
    let specifics =
      Punctuated::<SpecificType, Token![|]>::parse_separated_nonempty(&input)?
        .into_iter()
        .collect();
    let interfaces = {
      if input.peek(Token![;]) {
        input.parse::<Token![;]>()?;
        Punctuated::<SpecificType, Token![|]>::parse_terminated(&input)?
          .into_iter()
          .collect()
      } else {
        vec![]
      }
    };
    Ok(UnifiedType {
      unified: unified,
      specifics: specifics,
      interfaces: interfaces,
      vis: vis
    })
  }
}

struct SpecificType {
  path: TypePath,
}
impl Parse for SpecificType {
  fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
    let path = input.parse::<TypePath>()?;
    Ok(SpecificType { path: path })
  }
}
