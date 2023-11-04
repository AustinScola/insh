use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Error as SynError, Expr, ExprPath, Ident, Meta, MetaList, Path,
    PathArguments, PathSegment, Result as SynResult, Token,
};

pub struct FieldAttrArgs {
    pk: bool,
    default: Option<Expr>,
}

impl FieldAttrArgs {
    pub fn is_pk(&self) -> bool {
        self.pk
    }

    pub fn default(&self) -> &Option<Expr> {
        &self.default
    }
}

impl TryFrom<&Meta> for FieldAttrArgs {
    type Error = SynError;

    fn try_from(meta: &Meta) -> Result<Self, SynError> {
        let meta_list: &MetaList = match meta {
            Meta::List(meta_list) => meta_list,
            _ => {
                panic!("Expected list for `field` attribute.");
            }
        };

        let exprs: Punctuated<Expr, Token![,]> =
            match meta_list.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
                Ok(exprs) => exprs,
                Err(error) => {
                    return Err(SynError::new(
                        meta_list.span(),
                        "Expected comma list of expressions.",
                    ))
                }
            };

        let mut pk: bool = false;
        let mut default: Option<Expr> = None;
        for expr in exprs.iter() {
            match expr {
                Expr::Path(expr_path) => {
                    if expr_path.path.is_ident("pk") {
                        pk = true;
                    }
                }
                Expr::Assign(expr_assign) => {
                    let left: &Expr = &expr_assign.left;
                    if let Expr::Path(left_path) = left {
                        if left_path.path.is_ident("default") {
                            default = Some(*expr_assign.right.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(FieldAttrArgs {
            pk: pk,
            default: default,
        })
    }
}
