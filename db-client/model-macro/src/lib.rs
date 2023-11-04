use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::{Expr, Ident, Meta, MetaList};

mod field_attr_args;
use crate::field_attr_args::FieldAttrArgs;

#[proc_macro_attribute]
pub fn model(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the item into an AST.
    let mut derive_input: syn::DeriveInput = syn::parse(item).unwrap();

    // Only structs are supported.
    let struct_data: &mut syn::DataStruct = match derive_input.data {
        syn::Data::Struct(ref mut struct_data) => struct_data,
        syn::Data::Enum(_) => {
            return quote_spanned! {
                derive_input.span() => compile_error!("Models cannot be derived for enums, only structs.");
            }.into();
        }
        syn::Data::Union(_) => {
            return quote_spanned! {
                derive_input.span() => compile_error!("Models cannot be derived for unions, only structs.");
            }.into();
        }
    };

    // For each field, see if it has a "field" attribute.
    let mut pk_fields: Vec<syn::Field> = vec![];
    let mut non_pk_fields: Vec<syn::Field> = Vec::with_capacity(struct_data.fields.len() - 1);
    let mut non_pk_field_attr_args: Vec<Option<FieldAttrArgs>> =
        Vec::with_capacity(struct_data.fields.len() - 1);
    'loop_fields: for mut field in &mut struct_data.fields {
        for (index, attr) in field.attrs.iter().enumerate() {
            if attr.path().is_ident("field") {
                // Parse the field attribute arguments.
                let field_attr_args: FieldAttrArgs = match FieldAttrArgs::try_from(&attr.meta) {
                    Ok(field_attr_args) => field_attr_args,
                    Err(error) => {
                        return error.to_compile_error().into();
                    }
                };

                // Remove the attribute from the field because otherwise it would get processed as
                // a procedural attribute macro.
                field.attrs.remove(index);

                if field_attr_args.is_pk() {
                    pk_fields.push(field.clone());
                } else {
                    non_pk_fields.push(field.clone());
                    non_pk_field_attr_args.push(Some(field_attr_args));
                }

                continue 'loop_fields;
            }
        }

        non_pk_fields.push(field.clone());
        non_pk_field_attr_args.push(None);
    }

    // It is a compilation error if there is no primary key field.
    // TODO

    // It is a compilation error if there is more than one primary key field.
    // TODO

    // Modify the non-pk fields to have a builder attribute.
    let mut non_pk_fields_with_attrs: Vec<syn::Field> = Vec::with_capacity(non_pk_fields.len());
    for (non_pk_field, field_attr_args) in non_pk_fields.iter().zip(&non_pk_field_attr_args) {
        let mut non_pk_field_with_attrs: syn::Field = non_pk_field.clone();

        let setter_expr: TokenStream = quote! { setter(into) }.into();
        let setter_expr: Expr = parse_macro_input!(setter_expr as Expr);
        let mut builder_attr_exprs: Vec<Expr> = vec![setter_expr];
        if let Some(field_attr_args) = field_attr_args {
            if let Some(default_expr) = field_attr_args.default() {
                let default_expr: Expr = default_expr.clone();
                let default_expr: TokenStream = quote! { default=#default_expr }.into();
                let default_expr: Expr = parse_macro_input!(default_expr as Expr);
                builder_attr_exprs.push(default_expr);
            }
        }

        let attrs: TokenStream = quote! {
            #[builder(#(#builder_attr_exprs),*)]
        }
        .into();
        let attrs: Vec<syn::Attribute> = parse_macro_input!(attrs with syn::Attribute::parse_outer);

        non_pk_field_with_attrs.attrs = attrs;

        non_pk_fields_with_attrs.push(non_pk_field_with_attrs);
    }

    let name: &Ident = &derive_input.ident;

    let manager_mod_name_str: String = name.to_string().to_case(Case::Snake);
    let manager_mod_name: Ident = Ident::new(&manager_mod_name_str, name.span());

    let manager_name: Ident = format_ident!("{}Manager", name);

    let creator_type_name: Ident = format_ident!("{}Creator", name);

    let creator_builder_type_name = format_ident!("{}Builder", creator_type_name);

    // The type returned by `create` of the object manager is generic. The first type parameter is
    // the db client type. Then the rest are for the fields.
    let unit_types = vec![quote! {()}; non_pk_fields.len()];
    let creator_builder_type_name_params = quote! {((DbClientHandle,), #(#unit_types),*)};

    let gen = quote! {
        #derive_input

        use db_client::Model;

        impl Model for #name {
            type Objects = #manager_name;
        }

        mod #manager_mod_name {
            use typed_builder::TypedBuilder;

            use db_client::{fields, ObjectManager, DbClient, DbClientHandle};

            use super::#name;

            pub struct #manager_name {
                db_client_handle: DbClientHandle,
            }

            impl ObjectManager for #manager_name {
                type ModelType = #name;
                type CreatorType = #creator_type_name;
                type CreatorBuilderType = #creator_builder_type_name<#creator_builder_type_name_params>;

                fn new(db_client_handle: DbClientHandle) -> Self {
                    Self {
                        db_client_handle,
                    }
                }

                fn all() -> Vec<Self::ModelType> {
                    todo!();
                }

                fn creator(&self) -> #creator_builder_type_name<#creator_builder_type_name_params> {
                    Self::CreatorType::builder()._client_handle(self.db_client_handle)
                }
            }

            #[derive(TypedBuilder)]
            #[builder(build_method(name=create, into=#name))]
            pub struct #creator_type_name {
                // NOTE: Because of this field, models can't have a field named the same thing.
                pub _client_handle: DbClientHandle,

                #(#non_pk_fields_with_attrs),*
            }

            impl #creator_type_name {
            }
        }
        pub use #manager_mod_name::#manager_name;
        pub use #manager_mod_name::#creator_type_name;

        impl From<#creator_type_name> for #name {
            fn from(creator: #creator_type_name) -> Self {
                let request = db_api::Request::builder()
                    .params(db_api::RequestParams::Create(db_api::CreateRequestParams::builder().build()))
                    .build();
                creator._client_handle.request(request);
                todo!();
            }
        }
    };
    gen.into()
}
