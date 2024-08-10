use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};
use std::collections::HashSet;

fn parse_index_fields(attrs: &[syn::Attribute]) -> HashSet<String> {
    let mut fields = HashSet::new();

    for attr in attrs {
        if attr.path.is_ident("index_fields") {
            if let Ok(syn::Meta::List(meta_list)) = attr.parse_meta() {
                for nested in meta_list.nested.iter() {
                    if let syn::NestedMeta::Meta(syn::Meta::Path(path)) = nested {
                        if let Some(ident) = path.get_ident() {
                            fields.insert(ident.to_string());
                        }
                    }
                }
            }
        }
    }

    fields
}

pub fn impl_changing_trait(input: &DeriveInput) -> TokenStream {
    let struct_name = &input.ident;
    let indexing_struct_name_str = format!("Indexing{}", struct_name);
    let indexing_struct_name = syn::Ident::new(&indexing_struct_name_str, struct_name.span());
    let trait_name = syn::Ident::new(&format!("ChangingTraitFor{}", struct_name), struct_name.span());
    let db_type = quote! { simple_db::DataBase<#indexing_struct_name> };
    let mut offset = quote! { 0usize };

    // Парсинг атрибута index_fields для получения списка полей, для которых нужно генерировать полные методы
    let index_fields = parse_index_fields(&input.attrs);

    let (trait_methods, impl_methods): (Vec<_>, Vec<_>) = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => {
                fields_named
                    .named
                    .iter()
                    .map(|field| {
                        let field_name = field.ident.as_ref().unwrap();
                        let field_name_map = syn::Ident::new(&format!("{}_map", field_name), field_name.span());
                        let field_name_str = field_name.to_string();
                        let field_type = &field.ty;
                        let method_name = syn::Ident::new(
                            &format!("change_{}", field_name),
                            field_name.span(),
                        );

                        // Вычисление размера поля
                        let field_size = quote! { std::mem::size_of::<#field_type>() };

                        // Сохранение текущего смещения
                        let current_offset = offset.clone();

                        // Инкрементирование смещения для следующего поля
                        offset = quote! {#current_offset + std::mem::size_of::<#field_type>()};

                        // Генерация кода методов для полей, указанных в index_fields
                        if index_fields.contains(&field_name_str) {
                            // Сигнатура метода в трейте
                            let trait_method = quote! {
                                fn #method_name(&mut self, value: #field_type, index: usize) -> std::io::Result<()>;
                            };

                            // Реализация метода с индексированием
                            let impl_method = quote! {
                                fn #method_name(&mut self, value: #field_type, index: usize) -> std::io::Result<()> {
                                    let old_value = self.vec.get(index).unwrap().#field_name;
                                    if old_value == value {
                                        return Ok(());
                                    }

                                    let offset = #current_offset;
                                    let size = #field_size;

                                    let new_value_bytes = bytemuck::bytes_of(&value);
                                    std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start((std::mem::size_of::<<#indexing_struct_name as simple_db::IndexingTrait>::Type>() * index) as u64 + offset as u64))?;
                                    std::io::Write::write_all(&mut self.file, new_value_bytes)?;

                                    if let Some(set) = self.indexing.#field_name_map.get_mut(&old_value) {
                                        set.remove(&index);
                                        if set.is_empty() {
                                            self.indexing.#field_name_map.remove(&old_value);
                                        }
                                    }
                                    self.indexing.#field_name_map.entry(value).or_insert(std::collections::HashSet::new()).insert(index);

                                    let bytes = bytemuck::bytes_of_mut(&mut self.vec[index]);
                                    bytes[offset..offset + size].copy_from_slice(new_value_bytes);

                                    Ok(())
                                }
                            };

                            Some((trait_method, impl_method))
                        } else {
                            // Генерация кода для полей, которые не указаны в index_fields
                            let trait_method = quote! {
                                fn #method_name(&mut self, value: #field_type, index: usize) -> std::io::Result<()>;
                            };

                            // Реализация метода без индексирования
                            let impl_method = quote! {
                                fn #method_name(&mut self, value: #field_type, index: usize) -> std::io::Result<()> {
                                    let old_value = self.vec.get(index).unwrap().#field_name;
                                    if old_value == value {
                                        return Ok(());
                                    }

                                    let offset = #current_offset;
                                    let size = #field_size;

                                    let new_value_bytes = bytemuck::bytes_of(&value);

                                    std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start((std::mem::size_of::<<#indexing_struct_name as simple_db::IndexingTrait>::Type>() * index) as u64 + offset as u64))?;
                                    std::io::Write::write_all(&mut self.file, new_value_bytes)?;

                                    let bytes = bytemuck::bytes_of_mut(&mut self.vec[index]);
                                    bytes[offset..offset + size].copy_from_slice(new_value_bytes);

                                    Ok(())
                                }
                            };

                            Some((trait_method, impl_method))
                        }
                    })
                    .flatten()
                    .unzip()
            }
            _ => panic!("ChangingTrait can only be derived for structs with named fields"),
        },
        _ => panic!("ChangingTrait can only be derived for structs"),
    };

    let expanded = quote! {
        pub trait #trait_name {
            #(#trait_methods)*
        }

        impl #trait_name for #db_type {
            #(#impl_methods)*
        }
    };

    expanded.into()
}
