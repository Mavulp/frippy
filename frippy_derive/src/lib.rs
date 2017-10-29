
//! Provides the plugin derive macro

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

#[proc_macro_derive(PluginName)]
pub fn derive_plugin(data: TokenStream) -> TokenStream {
    let ast = syn::parse_derive_input(&data.to_string()).unwrap();
    let gen = expand_plugin(&ast);
    gen.parse().unwrap()
}

fn expand_plugin(ast: &syn::DeriveInput) -> quote::Tokens {
    let name = &ast.ident;
    let generics = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics PluginName for #name #ty_generics #where_clause {
            fn name(&self) -> &str {
                stringify!(#name)
            }
        }
    }
}
