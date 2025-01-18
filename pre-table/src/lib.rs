extern crate proc_macro;

use quote::quote;

use proc_macro2::{TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn freq_table(
	_attr: proc_macro::TokenStream,
	items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	const A0: f64 = 27.5;

	let mut items = TokenStream::from(items).into_iter();

	let mut tokens = TokenStream::new();

	let size: usize = std::u8::MAX as usize / 2 + 1;

	loop {
		match items.next() {
			None => panic!("No semi-colon found"),
			Some(tree) => match tree {
				TokenTree::Punct(ref punct) if punct.as_char() == ';' => {
					let vals = (0..size).map(|i| A0 * 2_f64.powf(((i as i8) as f64 - 21.0) / 12.0));
					tokens.extend(std::iter::once(quote! { = [ #(#vals),* ]; }));

					break;
				}
				_ => tokens.extend(std::iter::once(tree)),
			},
		}
	}

	tokens.extend(items);

	tokens.into()
}
