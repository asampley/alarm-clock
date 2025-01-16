extern crate proc_macro;

use std::path::{Path, PathBuf};

use quote::quote;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn dir_array(
	attr: proc_macro::TokenStream,
	items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let dir_name = parse_macro_input!(attr as syn::LitStr).value();

	let dir_path = Path::new(&dir_name);

	let dir = if dir_path.is_relative() {
        std::fs::read_dir(PathBuf::from_iter([
			Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()),
			dir_path,
		]))
	} else {
		std::fs::read_dir(dir_path)
	}.expect("Failed to read directory");

	let files = dir
		.into_iter()
		.map(|f| f.unwrap())
		.filter(|f| f.file_type().unwrap().is_file())
		.map(|f| f.path())
		.collect::<Vec<_>>();

	let mut items_iter = TokenStream::from(items).into_iter();

	let mut tokens = TokenStream::new();

	loop {
		match items_iter.next() {
			None => panic!("No array type found"),
			Some(tree) => match tree {
				TokenTree::Group(ref group) => if group.delimiter() == Delimiter::Bracket {
					let len = files.len();

					let file_structs: TokenStream = files
						.iter()
						.map(|p| {
							let name = p.file_stem().unwrap().to_str().unwrap();
							let path = p.as_path().to_str().unwrap();
							quote!( File { name: #name, data: include_bytes!( #path ) }, )
						})
						.collect();

					tokens.extend(quote!( [ File ; #len ] = [ #file_structs ] ));

					break;
				}
				_ => tokens.extend(std::iter::once(tree)),
			},
		}
	};

	tokens.extend(items_iter);

	proc_macro::TokenStream::from(tokens)
}
