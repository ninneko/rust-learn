use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, Meta, NestedMeta, Lit, Attribute};

// 文字列バリデーション用の設定を保持する構造体
struct StringValidation {
    min_length: Option<usize>,
    max_length: Option<usize>,
}

// 属性からバリデーション設定を解析する関数
fn parse_string_validation(attrs: &[Attribute]) -> StringValidation {
    let mut validation = StringValidation {
        min_length: None,
        max_length: None,
    };

    for attr in attrs {
        if attr.path.is_ident("validate") {
            if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
                for nested in meta_list.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(name_value)) = nested {
                        if name_value.path.is_ident("min_length") {
                            if let Lit::Int(lit) = &name_value.lit {
                                validation.min_length = lit.base10_parse().ok();
                            }
                        } else if name_value.path.is_ident("max_length") {
                            if let Lit::Int(lit) = &name_value.lit {
                                validation.max_length = lit.base10_parse().ok();
                            }
                        }
                    }
                }
            }
        }
    }

    validation
}

#[proc_macro_derive(RawStruct, attributes(validate))]
pub fn raw_struct_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident;
    let raw_struct_name = format!("Raw{}", struct_name);
    let raw_struct_ident = syn::Ident::new(&raw_struct_name, struct_name.span());

    let fields = match input.data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => &fields.named,
                _ => panic!("RawStruct は名前付きフィールドを持つ構造体のみサポートします"),
            }
        },
        _ => panic!("RawStruct は構造体のみサポートします"),
    };

    // すべてのフィールドをOption<String>として生成
    let raw_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        quote! {
            #field_name: Option<String>
        }
    });

    let validation_checks = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_type = &f.ty;
        let validation = parse_string_validation(&f.attrs);
        let is_optional = is_option_type(field_type);
        let base_type = if is_optional {
            get_inner_type(field_type).unwrap()
        } else {
            field_type
        };
        
        let required_check = if !is_optional {
            quote! {
                if self.#field_name.is_none() {
                    let mut err = validator::ValidationError::new(stringify!(#field_name));
                    err.message = Some(format!("フィールド '{}' は必須項目です", stringify!(#field_name)).into());
                    errors.add(stringify!(#field_name), err);
                }
            }
        } else {
            quote! {}
        };

        let type_str = quote!(#base_type).to_string();
        let value_check = match type_str.as_str() {
            "u8" | "u16" | "u32" | "u64" | "u128" => quote! {
                if let Some(value) = &self.#field_name {
                    match value {
                        v if v.starts_with('-') => {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' に負の値 ({}) が指定されましたが、{}型は負の値を受け付けません", 
                                stringify!(#field_name), 
                                value,
                                stringify!(#base_type)
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        },
                        v => match v.parse::<#base_type>() {
                            Ok(_) => {},
                            Err(e) => {
                                let mut err = validator::ValidationError::new(stringify!(#field_name));
                                err.message = Some(match e.to_string().contains("invalid digit") {
                                    true => format!("フィールド '{}' の値 ({}) が数値ではありません", 
                                        stringify!(#field_name),
                                        value
                                    ),
                                    false => format!("フィールド '{}' の値 ({}) が {}型の範囲（0 ～ {}) を超えています", 
                                        stringify!(#field_name),
                                        value,
                                        stringify!(#base_type),
                                        #base_type::MAX
                                    )
                                }.into());
                                errors.add(stringify!(#field_name), err);
                            }
                        }
                    }
                }
            },
            "i8" | "i16" | "i32" | "i64" | "i128" => quote! {
                if let Some(value) = &self.#field_name {
                    match value.parse::<#base_type>() {
                        Ok(_) => {},
                        Err(e) => {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(match e.to_string().contains("invalid digit") {
                                true => format!("フィールド '{}' の値 ({}) が数値ではありません", 
                                    stringify!(#field_name),
                                    value
                                ),
                                false => format!("フィールド '{}' の値 ({}) が {}型の範囲（{} ～ {}) を超えています", 
                                    stringify!(#field_name),
                                    value,
                                    stringify!(#base_type),
                                    #base_type::MIN,
                                    #base_type::MAX
                                )
                            }.into());
                            errors.add(stringify!(#field_name), err);
                        }
                    }
                }
            },
            "f32" | "f64" => quote! {
                if let Some(value) = &self.#field_name {
                    match value.parse::<#base_type>() {
                        Ok(_) => {},
                        Err(_) => {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の値 ({}) が有効な浮動小数点数ではありません", 
                                stringify!(#field_name),
                                value
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    }
                }
            },
            "bool" => quote! {
                if let Some(value) = &self.#field_name {
                    match value.to_lowercase().as_str() {
                        "true" | "false" | "1" | "0" => {},
                        _ => {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の値 ({}) が真偽値ではありません。'true'/'false' または '1'/'0' を使用してください", 
                                stringify!(#field_name),
                                value
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    }
                }
            },
            "String" => {
                let length_validation = match (validation.min_length, validation.max_length) {
                    (Some(min), Some(max)) => quote! {
                        let len = value.chars().count();
                        if len < #min || len > #max {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の文字数が制限（{} ～ {} 文字）を超えています（現在: {} 文字）", 
                                stringify!(#field_name),
                                #min,
                                #max,
                                len
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    },
                    (Some(min), None) => quote! {
                        let len = value.chars().count();
                        if len < #min {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の文字数が最小文字数（{} 文字）未満です（現在: {} 文字）", 
                                stringify!(#field_name),
                                #min,
                                len
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    },
                    (None, Some(max)) => quote! {
                        let len = value.chars().count();
                        if len > #max {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の文字数が最大文字数（{} 文字）を超えています（現在: {} 文字）", 
                                stringify!(#field_name),
                                #max,
                                len
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    },
                    (None, None) => quote! {}
                };

                quote! {
                    if let Some(value) = &self.#field_name {
                        #length_validation
                    }
                }
            },
            _ => quote! {
                if let Some(value) = &self.#field_name {
                    match value.parse::<#base_type>() {
                        Ok(_) => {},
                        Err(_) => {
                            let mut err = validator::ValidationError::new(stringify!(#field_name));
                            err.message = Some(format!("フィールド '{}' の値 ({}) が {}型として無効です", 
                                stringify!(#field_name),
                                value,
                                stringify!(#base_type)
                            ).into());
                            errors.add(stringify!(#field_name), err);
                        }
                    }
                }
            }
        };

        quote! {
            #required_check
            #value_check
        }
    });

    let expanded = quote! {
        #[derive(Debug, Clone, serde::Deserialize)]
        pub struct #raw_struct_ident {
            #(#raw_fields,)*
        }

        impl validator::Validate for #raw_struct_ident {
            fn validate(&self) -> Result<(), validator::ValidationErrors> {
                let mut errors = validator::ValidationErrors::new();
                
                #(#validation_checks)*

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    };

    TokenStream::from(expanded)
}

// 型がOption<T>かどうかをチェックする補助関数
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.first() {
            return segment.ident == "Option"
        }
    }
    false
}

// Option<T>の内部の型を取得する補助関数
fn get_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.first() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type);
                    }
                }
            }
        }
    }
    None
}
