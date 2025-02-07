use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument};

#[proc_macro_derive(RawStruct)]
pub fn raw_struct_derive(input: TokenStream) -> TokenStream {
    // 入力を解析
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident;
    let raw_struct_name = format!("Raw{}", struct_name);
    let raw_struct_ident = syn::Ident::new(&raw_struct_name, struct_name.span());

    // フィールドの処理
    let fields = match input.data {
        Data::Struct(ref data) => {
            match data.fields {
                Fields::Named(ref fields) => &fields.named,
                _ => panic!("RawStruct は名前付きフィールドを持つ構造体のみサポートします"),
            }
        },
        _ => panic!("RawStruct は構造体のみサポートします"),
    };

    // 新しいフィールドの生成
    let raw_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_type = &f.ty;
        
        // Option<T> の場合の処理
        let new_type = if is_option_type(field_type) {
            quote! { Option<String> }
        } else {
            quote! { String }
        };

        quote! {
            #field_name: #new_type
        }
    });

    // バリデーション用のフィールド名とその型の取得
    let validation_fields = fields.iter().map(|f| {
        let field_name = &f.ident;
        let field_type = if is_option_type(&f.ty) {
            get_inner_type(&f.ty).unwrap()
        } else {
            &f.ty
        };
        (field_name, field_type)
    });

    // バリデーションロジックの生成
    let validation_checks = validation_fields.map(|(field_name, field_type)| {
        quote! {
            if let Some(value) = &self.#field_name {
                if value.parse::<#field_type>().is_err() {
                    errors.add(
                        stringify!(#field_name),
                        validator::ValidationError::new("invalid_format")
                    );
                }
            }
        }
    });

    // 新しい構造体の生成（Deserializeトレイトを追加し、Validateを実装する）
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
