extern crate proc_macro;

use quote::quote;

use syn::{Expr, ExprLit, Lit, Type};

fn f64_table(
	mut f: impl FnMut(usize, usize) -> f64,
	items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let syn::ForeignItemStatic {
		attrs,
		vis,
		static_token,
		mutability,
		ident,
		colon_token,
		ty,
		semi_token,
	} = match syn::parse(items) {
		Ok(v) => v,
		Err(e) => return e.into_compile_error().into(),
	};

	let Type::Array(ref array_type) = *ty else {
		return syn::Error::new_spanned(ty, "Type must be an array")
			.into_compile_error()
			.into();
	};

	let Expr::Lit(ExprLit {
		lit: Lit::Int(ref size),
		..
	}) = array_type.len
	else {
		return syn::Error::new_spanned(&array_type.len, "Size must be an integer literal")
			.into_compile_error()
			.into();
	};

	let size = match size.base10_parse() {
		Ok(v) => v,
		Err(e) => {
			return syn::Error::new_spanned(&array_type.len, format!("Failed to parse usize: {e}"))
				.into_compile_error()
				.into();
		}
	};

	let vals: Vec<f64> = (0..size).map(|i| f(i, size)).collect();

	quote! {
		#(#attrs) * #vis #static_token #mutability #ident #colon_token #ty = [ #(#vals),* ] #semi_token
	}
	.into()
}

#[proc_macro_attribute]
pub fn sin_table(
	_attr: proc_macro::TokenStream,
	items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	f64_table(
		|i, size| i as f64 / size as f64 * core::f64::consts::TAU,
		items,
	)
}

#[proc_macro_attribute]
pub fn freq_table(
	_attr: proc_macro::TokenStream,
	items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	const A0: f64 = 27.5;

	f64_table(
		|i, _| A0 * 2_f64.powf(((i as i8) as f64 - 21.0) / 12.0),
		items,
	)
}
