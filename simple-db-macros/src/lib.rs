// extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields, Meta, NestedMeta};

#[proc_macro_derive(IndexingTraitImpl, attributes(index_fields))]
pub fn indexing_trait(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let struct_name = &derive_input.ident;

    ensure_repr_c(&derive_input.attrs);

    let indexing_trait_impl = generate_indexing_trait(&derive_input, struct_name);

    let expanded = quote! {
        #indexing_trait_impl
    };

    TokenStream::from(expanded)
}

fn ensure_repr_c(attrs: &[Attribute]) {
    let has_repr_c = attrs.iter().any(|attr| {
        matches!(attr.parse_meta(), Ok(Meta::List(meta)) if meta.path.is_ident("repr") && meta.nested.iter().any(|n| matches!(n, NestedMeta::Meta(Meta::Path(path)) if path.is_ident("C"))))
    });
    if !has_repr_c {
        panic!("This struct must be declared with #[repr(C)] to use IndexingTraitImpl");
    }
}

fn generate_indexing_trait(derive_input: &DeriveInput, struct_name: &proc_macro2::Ident) -> proc_macro2::TokenStream {
    let mut field_names_to_index = vec![];
    for attr in &derive_input.attrs {
        if attr.path.is_ident("index_fields") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in meta_list.nested.iter() {
                    if let NestedMeta::Meta(Meta::Path(path)) = nested {
                        if let Some(ident) = path.get_ident() {
                            field_names_to_index.push(ident.clone());
                        }
                    }
                }
            }
        }
    }

    if let Data::Struct(data_struct) = &derive_input.data {
        let indexing_struct_name = format_ident!("Indexing{}", struct_name);
        let mut hashmap_fields = proc_macro2::TokenStream::new();
        let mut add_entries = proc_macro2::TokenStream::new();
        let mut remove_entries = proc_macro2::TokenStream::new();

        match &data_struct.fields {
            Fields::Named(named_fields) => {
                for field in &named_fields.named {
                    let field_name = &field.ident;
                    let field_type = &field.ty;
                    if field_name.is_some() && field_names_to_index.contains(field_name.as_ref().unwrap()) {
                        let hashmap_field = format_ident!("{}_map", field_name.as_ref().unwrap());
                        hashmap_fields.extend(quote! {
                            pub #hashmap_field: std::collections::HashMap<#field_type, std::collections::HashSet<usize>>,
                        });

                        add_entries.extend(quote! {
                            self.#hashmap_field.entry(elem.#field_name.clone()).or_insert(std::collections::HashSet::new()).insert(index);
                        });

                        remove_entries.extend(quote! {
                            if let Some(set) = self.#hashmap_field.get_mut(&elem.#field_name) {
                                set.remove(&index);
                        
                                // If the HashSet is now empty, remove the entire key-value pair
                                if set.is_empty() {
                                    self.#hashmap_field.remove(&elem.#field_name);
                                }
                            }
                        });
                    }
                }
            }
            _ => panic!("This macro only supports named fields"),
        }

        let indexing_struct = quote! {
            #[derive(Default, Debug)]
            pub struct #indexing_struct_name {
                #hashmap_fields
            }

            impl simple_db::IndexingTrait for #indexing_struct_name {
                type Type = #struct_name;

                fn add(&mut self, elem: &Self::Type, index: usize) {
                    #add_entries
                }

                fn remove(&mut self, elem: &Self::Type, index: usize) {
                    #remove_entries
                }
            }
        };

        indexing_struct
    } else {
        panic!("This macro can only be used with structs");
    }
}


mod field_methods;

#[proc_macro_derive(FieldMethods)]
pub fn field_methods_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    field_methods::impl_changing_trait(&input).into()
}
