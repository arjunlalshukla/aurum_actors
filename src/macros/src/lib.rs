extern crate proc_macro;
use proc_macro::TokenStream;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

mod aurum_interface;

#[proc_macro_derive(AurumInterface, attributes(aurum))]
pub fn aurum_interface(item: TokenStream) -> TokenStream {
  aurum_interface::aurum_interface_impl(syn::parse(item).unwrap())
}

fn write_and_fmt<S: ToString>(file: String, code: S) -> io::Result<()> {
  fs::create_dir_all("./code-gen/")?;
  let p = Path::new("./code-gen").join(format!("{}.rs", file));
  fs::write(p.clone(), code.to_string())?;
  Command::new("rustfmt").arg(p).spawn()?.wait()?;
  Ok(())
}
