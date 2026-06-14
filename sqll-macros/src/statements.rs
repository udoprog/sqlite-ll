use core::cell::RefCell;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{Error, Expr, ExprLit, Ident, ItemStruct, Lit, LitStr, Member};

pub(super) struct Ctxt {
    errors: RefCell<Vec<Error>>,
}

impl Ctxt {
    fn capture<T>(&self, res: Result<T, Error>) -> Result<T, ()> {
        match res {
            Ok(val) => Ok(val),
            Err(err) => {
                self.errors.borrow_mut().push(err);
                Err(())
            }
        }
    }

    fn error(&self, err: Error) {
        self.errors.borrow_mut().push(err);
    }
}

fn cx() -> Ctxt {
    Ctxt {
        errors: RefCell::new(Vec::new()),
    }
}

pub(super) fn expand(input: TokenStream) -> TokenStream {
    let cx = cx();

    let errors = 'errors: {
        if let Ok(stream) = inner(&cx, input) {
            let errors = cx.errors.into_inner();

            if !errors.is_empty() {
                break 'errors errors;
            }

            return stream;
        }

        cx.errors.into_inner()
    };

    debug_assert!(!errors.is_empty());
    let mut out = TokenStream::new();

    for err in errors {
        out.extend(err.to_compile_error());
    }

    out
}

fn inner(cx: &Ctxt, input: TokenStream) -> Result<TokenStream, ()> {
    let st: ItemStruct = cx.capture(syn::parse2(input))?;

    let pool_error = quote!(::sqll::PoolError);
    let send_statement = quote!(::sqll::SendStatement);
    let is_read_only_t = quote!(::sqll::IsReadOnly);
    let statements_t = quote!(::sqll::Statements);
    let from = quote!(::core::convert::From);

    let mut read_only = false;

    for a in &st.attrs {
        if !a.path().is_ident("sql") {
            continue;
        }

        let result = a.parse_nested_meta(|s: ParseNestedMeta<'_>| {
            if s.path.is_ident("read_only") {
                read_only = true;
                return Ok(());
            }

            Err(Error::new_spanned(
                &s.path,
                "unsupported attribute, expected `read_only`",
            ))
        });

        if let Err(error) = result {
            cx.error(error);
        }
    }

    let mut members = Vec::new();
    let mut variables = Vec::new();
    let mut builders = Vec::new();

    for (index, f) in st.fields.iter().enumerate() {
        let span = match f.ident {
            Some(ref name) => name.span(),
            None => f.span(),
        };

        let member = match f.ident {
            Some(ref name) => Member::from(name.clone()),
            None => Member::from(index),
        };

        match f.ident {
            Some(ref name) => {
                variables.push(name.clone());
            }
            None => {
                variables.push(Ident::new(&format!("_{index}"), span));
            }
        }

        let mut query = QueryString::default();

        for a in &f.attrs {
            if !a.path().is_ident("sql") {
                continue;
            }

            let Ok(name_value) = cx.capture(a.meta.require_name_value()) else {
                continue;
            };

            if let Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) = &name_value.value
            {
                query.append(lit);
                continue;
            }

            cx.error(Error::new_spanned(name_value, "expected string literal"));
        }

        members.push(member);

        let query = query.output;
        let ty = &f.ty;

        let read_only_method;
        let member;
        let prepare_failed;
        let not_thread_safe;

        match f.ident {
            Some(ref name) => {
                let message = name.to_string();
                read_only_method = Ident::new("field_not_read_only", Span::call_site());
                prepare_failed = Ident::new("field_prepare_failed", Span::call_site());
                not_thread_safe = Ident::new("field_not_thread_safe", Span::call_site());
                member = quote!(#message);
            }
            None => {
                read_only_method = Ident::new("index_not_read_only", Span::call_site());
                prepare_failed = Ident::new("index_prepare_failed", Span::call_site());
                not_thread_safe = Ident::new("index_not_thread_safe", Span::call_site());
                member = quote!(#index);
            }
        };

        let read_only = read_only.then(|| {
            quote! {
                if !stmt.is_read_only() {
                    return Err(#pool_error::#read_only_method(#member));
                }
            }
        });

        builders.push(quote! {{
            let stmt = match c.prepare_with(#query).persistent().build() {
                Ok(stmt) => stmt,
                Err(error) => {
                    return Err(#pool_error::#prepare_failed(#member, error));
                }
            };

            let stmt = match unsafe { stmt.into_send() } {
                Ok(stmt) => stmt,
                Err(error) => {
                    return Err(#pool_error::#not_thread_safe(#member, error));
                }
            };

            #read_only

            <#ty as #from<#send_statement>>::from(stmt)
        }});
    }

    let ident = &st.ident;

    let impl_read_only = read_only.then(|| {
        quote! {
            unsafe impl #is_read_only_t for #ident {}
        }
    });

    Ok(quote! {
        impl #statements_t for #ident {
            fn build(c: ::sqll::Connection) -> Result<Self, #pool_error> {
                #(let #variables = #builders;)*

                Ok(Self {
                    #(#members: #variables,)*
                })
            }
        }

        #impl_read_only
    })
}

#[derive(Default)]
struct QueryString {
    output: String,
}

impl QueryString {
    fn append(&mut self, s: &LitStr) {
        let s = s.value();
        let s = s.trim();

        if s.is_empty() {
            return;
        }

        if !self.output.is_empty() {
            self.output.push(' ');
        }

        self.output.push_str(s);
    }
}
