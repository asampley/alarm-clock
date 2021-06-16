extern crate proc_macro;
use std::iter::FromIterator;
use std::f32;

use proc_macro::{
	TokenStream,
	TokenTree,
	Delimiter,
	Spacing,
	Literal,
	Ident,
	Punct,
	Group,
	Span,
};

#[proc_macro]
pub fn freq_table(items: TokenStream) -> TokenStream {
	const A0: f64 = 27.5;

	let mut items = items.into_iter();

	let name = match items.next().expect("Missing table identifier") {
		TokenTree::Ident(i) => i,
		_ => panic!("Invalid table identifier"),
	};

	let size: usize = std::u8::MAX as usize + 1;

	array( name, "f64", size,
		|i| TokenTree::from(Literal::f64_unsuffixed(A0 * 2_f64.powf( ( (i as i8) as f64 - 21.0) / 12.0 )))
	)
}

#[proc_macro]
pub fn sin_table(items: TokenStream) -> TokenStream {
	let mut items = items.into_iter();

	let name = match items.next().expect("Missing table identifier") {
		TokenTree::Ident(i) => i,
		_ => panic!("Invalid table identifier"),
	};
	let elements = match items.next().expect("Missing table size") {
		TokenTree::Literal(l) => l,
		_ => panic!("Table size must be a literal"),
	};

	let size: usize = elements.to_string().parse().expect("Expected usize literal");

	array( name, "f32", size,
		|i| TokenTree::from(Literal::f32_unsuffixed(f32::sin(i as f32 / size as f32 * f32::consts::PI * 2.0)))
	)
}

fn array(name: Ident, typ: &str, size: usize, arr_value: impl Fn(usize) -> TokenTree) -> TokenStream {
	let mut tokens: Vec<TokenTree> = Vec::new();

	let decl: Vec<TokenTree> = vec![
		Ident::new("const", Span::call_site()).into(),
		name.into(),
		Punct::new(':', Spacing::Alone).into(),
	];

	for t in decl {
		tokens.push(t);
	}

	tokens.push(array_type(typ, size));
	tokens.push(Punct::new('=', Spacing::Alone).into());
	tokens.push(array_value(size, |i| arr_value(i)));
	tokens.push(Punct::new(';', Spacing::Alone).into());

	TokenStream::from_iter(tokens.into_iter())
}

fn array_type(typ: &str, size: usize) -> TokenTree {
	let typ = Ident::new(typ, Span::call_site());

	let arr_type: Vec<TokenTree> = vec![
		typ.into(),
		Punct::new(';', Spacing::Alone).into(),
		Literal::usize_unsuffixed(size).into(),
	];
	
	TokenTree::from(Group::new(Delimiter::Bracket, TokenStream::from_iter(arr_type.into_iter())))
}

fn array_value (size: usize, to_token_tree: impl Fn(usize) -> TokenTree) -> TokenTree {
	TokenTree::from(Group::new(Delimiter::Bracket, 
		(0..size)
			.flat_map(|a| vec![
				to_token_tree(a),
				TokenTree::from(Punct::new(',', Spacing::Alone)),
			].into_iter())
			.collect()
	)).into()
}
