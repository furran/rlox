use core::panic;

use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[proc_macro_derive(Trace)]
pub fn trace_macro_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();

    let name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = ast.generics.split_for_impl();
    let trace_body = match &ast.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields_named) => {
                let traces = fields_named.named.iter().map(|field| {
                    let idx = field.ident.as_ref().unwrap();
                    quote! { self.#idx.trace(); }
                });
                quote! { #(#traces)* }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                let traces = fields_unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                    let idx = syn::Index::from(i);
                    quote! { self.#idx.trace(); }
                });
                quote! { #(#traces)* }
            }
            syn::Fields::Unit => quote! {},
        },
        syn::Data::Enum(data_enum) => {
            let arms = data_enum.variants.iter().map(|variant| {
                let variant_name = &variant.ident;
                match &variant.fields {
                    syn::Fields::Named(fields_named) => {
                        let field_names: Vec<_> =
                            fields_named.named.iter().map(|f| &f.ident).collect();
                        quote! {
                            Self::#variant_name { #(#field_names), * } => {
                                #(#field_names.trace();)*
                            }
                        }
                    }
                    syn::Fields::Unnamed(fields_unnamed) => {
                        let bindings: Vec<_> = (0..fields_unnamed.unnamed.len())
                            .map(|i| quote::format_ident!("f{}", i))
                            .collect();

                        quote! {
                            Self::#variant_name(#(#bindings),*) => {
                                #(#bindings.trace();)*
                            }
                        }
                    }
                    syn::Fields::Unit => quote! { Self::#variant_name => {}, },
                }
            });

            quote! { match self { #(#arms)* } }
        }
        syn::Data::Union(_) => panic!("Trace cannot be derived for a union"),
    };

    quote! {
        impl #impl_generics ::rlox_gc::Trace for #name #type_generics #where_clause {
            fn trace(&self) {
                #trace_body
            }
        }
    }
    .into()
}
