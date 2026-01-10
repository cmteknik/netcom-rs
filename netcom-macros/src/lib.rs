extern crate proc_macro;
use proc_macro::TokenStream;
// use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse::Parse, parse_macro_input, Data, DeriveInput, Expr, Fields, Lit, Meta};

#[proc_macro_derive(NetcomMap, attributes(param))]
pub fn netcom_map_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let mut metadata_entries = vec![];
    let mut to_wrops_entries = vec![];
    let mut to_rdops_entries = vec![];
    let mut apply_result_entries = vec![];
    let mut field_idents = vec![];

    if let Data::Struct(data_struct) = input.data {
        if let Fields::Named(fields) = data_struct.fields {
            for field in fields.named {
                let field_name = field.ident.unwrap();
                let field_name_str = field_name.to_string();
                let mut param_value: Option<String> = None;

                for attr in field.attrs {
                    if attr.path().is_ident("param") {
                        if let Ok(meta) = attr.parse_args_with(syn::Meta::parse) {
                            if let Meta::NameValue(name_value) = meta {
                                if name_value.path.is_ident("p") {
                                    if let Expr::Lit(expr_lit) = name_value.value {
                                        if let Lit::Str(lit_str) = expr_lit.lit {
                                            param_value = Some(lit_str.value());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(param_value) = param_value {
                    metadata_entries.push(quote! {
                        map.insert(#field_name_str.to_string(), #param_value.to_string());
                    });

                    to_rdops_entries.push(quote! {
                        RdOp::Default {
                            p: #param_value.to_string(),
                        }
                    });

                    to_wrops_entries.push(quote! {
                        WrOp::Default {
                            p: #param_value.to_string(),
                            v: self.#field_name,
                        }
                    });

                    apply_result_entries.push(quote! {
                        if let Some(v) = result.get(#param_value) {
                            if let Some(vv) = v {
                                self.#field_name = *vv;
                            }
                        }
                    });

                    field_idents.push(quote! {
                        #field_name: 0.0
                    });
                }
            }
        }
    }

    let expanded = quote! {
        impl #struct_name {
            pub fn metadata() -> HashMap<String, String> {
                let mut map = HashMap::new();
                #(#metadata_entries)*
                map
            }
        }

        impl NetcomSync for #struct_name {
            fn to_rdops(&self) -> Vec<RdOp> {
                vec![#(#to_rdops_entries),*]
            }

            fn to_wrops(&self) -> Vec<WrOp> {
                vec![#(#to_wrops_entries),*]
            }

            fn apply_result(&mut self, result: &HashMap<String, Option<f64>>) {
                #(#apply_result_entries)*
            }
        }
    };

    TokenStream::from(expanded)
}
