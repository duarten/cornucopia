use core::str;
use std::fmt::{Formatter, Write};

use heck::ToUpperCamelCase;
use indexmap::IndexMap;
use quote::quote;

use crate::{
    prepare_queries::{
        Preparation, PreparedContent, PreparedField, PreparedItem, PreparedModule, PreparedQuery,
        PreparedType,
    },
    utils::Lazy,
    CodegenSettings,
};

impl PreparedField {
    pub fn own_struct(&self) -> String {
        let it = self.ty.own_ty(self.is_inner_nullable);
        if self.is_nullable {
            format!("Option<{}>", it)
        } else {
            it
        }
    }

    pub fn param_ergo_ty(&self, is_async: bool, traits: &mut Vec<String>) -> String {
        let it = self
            .ty
            .param_ergo_ty(self.is_inner_nullable, is_async, traits);
        if self.is_nullable {
            format!("Option<{}>", it)
        } else {
            it
        }
    }

    pub fn param_ty(&self, is_async: bool) -> String {
        let it = self.ty.param_ty(self.is_inner_nullable, is_async);
        if self.is_nullable {
            format!("Option<{}>", it)
        } else {
            it
        }
    }

    pub fn brw_ty(&self, has_lifetime: bool, is_async: bool) -> String {
        let it = self
            .ty
            .brw_ty(self.is_inner_nullable, has_lifetime, is_async);
        if self.is_nullable {
            format!("Option<{}>", it)
        } else {
            it
        }
    }

    pub fn owning_call(&self, name: Option<&str>) -> String {
        self.ty.owning_call(
            name.unwrap_or(&self.name),
            self.is_nullable,
            self.is_inner_nullable,
        )
    }

    pub fn owning_assign(&self) -> String {
        let call = self.owning_call(None);
        if call == self.name {
            call
        } else {
            format!("{}: {}", self.name, call)
        }
    }
}

fn struct_tosql(
    w: &mut impl Write,
    struct_name: &str,
    fields: &[PreparedField],
    name: &str,
    is_borrow: bool,
    is_params: bool,
    is_async: bool,
) {
    let (struct_name, lifetime) = if is_borrow {
        if is_params {
            (format!("{struct_name}Borrowed"), "<'a>")
        } else {
            (format!("{struct_name}Params"), "<'a>")
        }
    } else {
        (struct_name.to_string(), "")
    };
    let field_names = fields.iter().map(|p| &p.name);
    let write_names = fields.iter().map(|p| format!("\"{}\"", &p.name));
    let write_ty = fields.iter().map(|p| p.ty.sql_wrapped(&p.name, is_async));
    let accept_names = write_names.clone();
    let accept_ty = fields.iter().map(|p| p.ty.accept_to_sql(is_async));
    let name = format!("\"{name}\"");
    let nb_fields = format!("{}usize", fields.len());

    quote!(w =>
        impl<'a> postgres_types::ToSql for #struct_name #lifetime {
            fn to_sql(
                &self,
                ty: &postgres_types::Type,
                out: &mut postgres_types::private::BytesMut,
            ) -> Result<postgres_types::IsNull, Box<dyn std::error::Error + Sync + Send>,> {
                let #struct_name {
                    #(#field_names),*
                } = self;
                let fields = match *ty.kind() {
                    postgres_types::Kind::Composite(ref fields) => fields,
                    _ => unreachable!(),
                };
                out.extend_from_slice(&(fields.len() as i32).to_be_bytes());
                for field in fields {
                    out.extend_from_slice(&field.type_().oid().to_be_bytes());
                    let base = out.len();
                    out.extend_from_slice(&[0; 4]);
                    let r = match field.name() {
                        #(#write_names => postgres_types::ToSql::to_sql(#write_ty,field.type_(), out),)*
                        _ => unreachable!()
                    };
                    let count = match r? {
                        postgres_types::IsNull::Yes => -1,
                        postgres_types::IsNull::No => {
                            let len = out.len() - base - 4;
                            if len > i32::max_value() as usize {
                                return Err(Into::into("value too large to transmit"));
                            }
                            len as i32
                        }
                    };
                    out[base..base + 4].copy_from_slice(&count.to_be_bytes());
                }
                Ok(postgres_types::IsNull::No)
            }
            fn accepts(ty: &postgres_types::Type) -> bool {
                if ty.name() != #name {
                    return false;
                }
                match *ty.kind() {
                    postgres_types::Kind::Composite(ref fields) => {
                        if fields.len() != #nb_fields {
                            return false;
                        }
                        fields.iter().all(|f| match f.name() {
                            #(#accept_names => <#accept_ty as postgres_types::ToSql>::accepts(f.type_()),)*
                            _ => false,
                        })
                    }
                    _ => false,
                }
            }
            fn to_sql_checked(
                &self,
                ty: &postgres_types::Type,
                out: &mut postgres_types::private::BytesMut,
            ) -> Result<postgres_types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
                postgres_types::__to_sql_checked(self, ty, out)
            }
        }
    );
}

fn composite_fromsql(
    w: &mut impl Write,
    struct_name: &str,
    fields: &[PreparedField],
    name: &str,
    schema: &str,
) {
    let field_names = fields.iter().map(|p| &p.name);
    let read_names = field_names.clone();
    let read_idx = 0..fields.len();
    let struct_name = format!("{struct_name}Borrowed");
    let name = format!("\"{name}\"");
    let schema = format!("\"{schema}\"");

    quote!(w =>
        impl<'a> postgres_types::FromSql<'a> for #struct_name<'a> {
            fn from_sql(ty: &postgres_types::Type, out: &'a [u8]) ->
                Result<#struct_name<'a>, Box<dyn std::error::Error + Sync + Send>>
            {
                let fields = match *ty.kind() {
                    postgres_types::Kind::Composite(ref fields) => fields,
                    _ => unreachable!(),
                };
                let mut out = out;
                let num_fields = postgres_types::private::read_be_i32(&mut out)?;
                if num_fields as usize != fields.len() {
                    return std::result::Result::Err(
                        std::convert::Into::into(format!("invalid field count: {} vs {}", num_fields, fields.len())));
                }
                #(
                    let _oid = postgres_types::private::read_be_i32(&mut out)?;
                    let #read_names = postgres_types::private::read_value(fields[#read_idx].type_(), &mut out)?;
                )*
                Ok(#struct_name { #(#field_names),* })
            }

            fn accepts(ty: &postgres_types::Type) -> bool {
                ty.name() == #name && ty.schema() == #schema
            }
        }
    );
}

fn gen_params_struct(w: &mut impl Write, params: &PreparedItem, settings: CodegenSettings) {
    let PreparedItem {
        name,
        fields,
        is_copy,
        is_named,
        is_ref,
    } = params;
    let is_async = settings.is_async;
    if *is_named {
        let name = name.to_string();
        let traits = &mut Vec::new();

        let copy = if *is_copy { "Clone,Copy," } else { "" };
        let lifetime = if *is_ref { "'a," } else { "" };
        let fields_ty = fields
            .iter()
            .map(|p| p.param_ergo_ty(is_async, traits))
            .collect::<Vec<_>>();
        let fields_name = fields.iter().map(|p| &p.name);
        let traits_idx = (1..=traits.len()).into_iter().map(idx_char);
        quote!(w =>
            #[derive(#copy Debug)]
            pub struct #name<#lifetime #(#traits_idx: #traits),*> {
                #(pub #fields_name: #fields_ty),*
            }
        );
    }
}

fn gen_row_structs(
    w: &mut impl Write,
    row: &PreparedItem,
    CodegenSettings {
        is_async,
        derive_ser,
    }: CodegenSettings,
) {
    let PreparedItem {
        name,
        fields,
        is_copy,
        is_named,
        ..
    } = row;
    if *is_named {
        // Generate row struct
        let fields_name = fields.iter().map(|p| &p.name);
        let fields_ty = fields.iter().map(|p| p.own_struct());
        let copy = if *is_copy { "Copy" } else { "" };
        let ser_str = if derive_ser { "serde::Serialize," } else { "" };
        quote!(w =>
            #[derive(#ser_str Debug, Clone, PartialEq,#copy)]
            pub struct #name {
                #(pub #fields_name : #fields_ty),*
            }
        );

        if !is_copy {
            let fields_name = fields.iter().map(|p| &p.name);
            let fields_ty = fields.iter().map(|p| p.brw_ty(true, is_async));
            let from_name = fields_name.clone();
            let from_own_assign = fields.iter().map(|f| f.owning_assign());
            let brw_name = format!("{name}Borrowed");
            quote!(w =>
                pub struct #brw_name<'a> {
                    #(pub #fields_name : #fields_ty),*
                }
                impl<'a> From<#brw_name<'a>> for #name {
                    fn from(#brw_name { #(#from_name),* }: #brw_name<'a>) -> Self {
                        Self {
                            #(#from_own_assign),*
                        }
                    }
                }
            );
        };
    }
    {
        // Generate query struct
        let borrowed_str = if *is_copy { "" } else { "Borrowed" };
        let (client_mut, fn_async, fn_await, backend, collect, raw_type, raw_pre, raw_post, client) =
            if is_async {
                (
                    "",
                    "async",
                    ".await",
                    "tokio_postgres",
                    "try_collect().await",
                    "futures::Stream",
                    "",
                    ".into_stream()",
                    "cornucopia_async",
                )
            } else {
                (
                    "mut",
                    "",
                    "",
                    "postgres",
                    "collect()",
                    "Iterator",
                    ".iterator()",
                    "",
                    "cornucopia_sync",
                )
            };

        let row_struct = if *is_named {
            format!("{name}{borrowed_str}")
        } else {
            fields[0].brw_ty(false, is_async)
        };
        let name = format!("{name}Query");

        quote!(w =>
        pub struct #name<'a, C: GenericClient, T, const N: usize> {
            client: &'a #client_mut C,
            params: [&'a (dyn postgres_types::ToSql + Sync); N],
            stmt: &'a mut #client::private::Stmt,
            extractor: fn(&#backend::Row) -> #row_struct,
            mapper: fn(#row_struct) -> T,
        }
        impl<'a, C, T:'a, const N: usize> #name<'a, C, T, N> where C: GenericClient {
            pub fn map<R>(self, mapper: fn(#row_struct) -> R) -> #name<'a,C,R,N> {
                #name {
                    client: self.client,
                    params: self.params,
                    stmt: self.stmt,
                    extractor: self.extractor,
                    mapper,
                }
            }

            pub #fn_async fn one(self) -> Result<T, #backend::Error> {
                let stmt = self.stmt.prepare(self.client)#fn_await?;
                let row = self.client.query_one(stmt, &self.params)#fn_await?;
                Ok((self.mapper)((self.extractor)(&row)))
            }

            pub #fn_async fn all(self) -> Result<Vec<T>, #backend::Error> {
                self.iter()#fn_await?.#collect
            }

            pub #fn_async fn opt(self) -> Result<Option<T>, #backend::Error> {
                let stmt = self.stmt.prepare(self.client)#fn_await?;
                Ok(self
                    .client
                    .query_opt(stmt, &self.params)
                    #fn_await?
                    .map(|row| (self.mapper)((self.extractor)(&row))))
            }

            pub #fn_async fn iter(
                self,
            ) -> Result<impl #raw_type<Item = Result<T, #backend::Error>> + 'a, #backend::Error> {
                let stmt = self.stmt.prepare(self.client)#fn_await?;
                let it = self
                    .client
                    .query_raw(stmt, #client::private::slice_iter(&self.params))
                    #fn_await?
                    #raw_pre
                    .map(move |res| res.map(|row| (self.mapper)((self.extractor)(&row))))
                    #raw_post;
                Ok(it)
            }
        });
    }
}

pub fn idx_char(idx: usize) -> String {
    format!("T{idx}")
}

fn gen_query_fn(
    w: &mut impl Write,
    module: &PreparedModule,
    query: &PreparedQuery,
    CodegenSettings { is_async, .. }: CodegenSettings,
) {
    let PreparedQuery {
        name,
        row,
        sql,
        param,
    } = query;

    let (client_mut, fn_async, fn_await, backend, client) = if is_async {
        ("", "async", ".await", "tokio_postgres", "cornucopia_async")
    } else {
        ("mut", "", "", "postgres", "cornucopia_sync")
    };

    let struct_name = name.to_upper_camel_case();
    let stmt_name = format!("{struct_name}Stmt");
    let (param, param_field, order) = match param {
        Some((idx, order)) => {
            let it = module.params.get_index(*idx).unwrap().1;
            (Some(it), it.fields.as_slice(), order.as_slice())
        }
        None => (None, [].as_slice(), [].as_slice()),
    };
    let traits = &mut Vec::new();
    let params_ty: Vec<_> = order
        .iter()
        .map(|idx| param_field[*idx].param_ergo_ty(is_async, traits))
        .collect();
    let params_name = order.iter().map(|idx| &param_field[*idx].name);
    let traits_idx = (1..=traits.len()).into_iter().map(idx_char);
    let lazy_impl = Lazy::new(|w| {
        if let Some((idx, index)) = row {
            let PreparedItem {
                name: row_name,
                fields,
                is_copy,
                is_named,
                ..
            } = &module.rows.get_index(*idx).unwrap().1;
            // Query fn
            let params_name = params_name.clone();
            let params_name2 = params_name.clone();
            let nb_params = param_field.len();
            let traits_idx = traits_idx.clone();

            // TODO find a way to clean this mess
            let (row_struct_name, extractor, mapper): (
                String,
                Lazy<Box<dyn Fn(&mut Formatter)>>,
                String,
            ) = if *is_named {
                (
                    row_name.value.clone(),
                    Lazy::new(Box::new(|w: &mut Formatter| {
                        let name = if *is_copy {
                            row_name.to_string()
                        } else {
                            format!("{row_name}Borrowed")
                        };
                        let fields_name = fields.iter().map(|p| &p.name);
                        let fields_idx = (0..fields.len()).map(|i| index[i]);
                        quote!(w => #name {
                            #(#fields_name: row.get(#fields_idx)),*
                        })
                    })),
                    format!("<{row_name}>::from(it)"),
                )
            } else {
                let field = &fields[0];
                (
                    field.own_struct(),
                    Lazy::new(Box::new(|w: &mut Formatter| quote!(w => row.get(0)))),
                    field.owning_call(Some("it")),
                )
            };
            let query_name = format!("{row_name}Query");
            quote!(w =>
                pub fn bind<'a, C: GenericClient,#(#traits_idx: #traits),*>(&'a mut self, client: &'a #client_mut C, #(#params_name: &'a #params_ty),* ) -> #query_name<'a,C, #row_struct_name, #nb_params> {
                    #query_name {
                        client,
                        params: [#(#params_name2),*],
                        stmt: &mut self.0,
                        extractor: |row| { #extractor },
                        mapper: |it| { #mapper },
                    }
                }
            );
        } else {
            // Execute fn
            let params_wrap = order.iter().map(|idx| {
                let p = &param_field[*idx];
                p.ty.sql_wrapped(&p.name, is_async)
            });
            let traits_idx = traits_idx.clone();
            let params_name = params_name.clone();
            quote!(w =>
                pub #fn_async fn bind<'a, C: GenericClient,#(#traits_idx: #traits),*>(&'a mut self, client: &'a #client_mut C, #(#params_name: &'a #params_ty),*) -> Result<u64, #backend::Error> {{
                    let stmt = self.0.prepare(client)#fn_await?;
                    client.execute(stmt, &[ #(#params_wrap),* ])#fn_await
                }}
            );
        }
    });
    // Gen statement struct
    {
        let sql = sql.replace('"', "\\\""); // Rust string format escaping
        let sql = format!("\"{sql}\"");
        quote!(w =>
            pub fn #name() -> #stmt_name {
                #stmt_name(#client::private::Stmt::new(#sql))
            }
            pub struct #stmt_name(#client::private::Stmt);
            impl #stmt_name {
                #lazy_impl
            }
        );
    }

    // Param impl
    if let Some(param) = param {
        let traits_idx2 = traits_idx.clone();
        let traits_idx3 = traits_idx.clone();

        if param.is_named {
            let param_name = &param.name;
            let lifetime = if param.is_copy || !param.is_ref {
                ""
            } else {
                "'a,"
            };
            if let Some((idx, _)) = row {
                let prepared_row = &module.rows.get_index(*idx).unwrap().1;
                let name = prepared_row.name.value.clone();
                let query_row_struct = if prepared_row.is_named {
                    name
                } else {
                    prepared_row.fields[0].own_struct()
                };
                let name = &module.rows.get_index(*idx).unwrap().1.name;
                let nb_params = param_field.len();
                let query_name = format!("{name}Query");
                quote!(w =>
                    impl <'a, C: GenericClient,#(#traits_idx: #traits),*> #client::Params<'a, #param_name<#lifetime #(#traits_idx2),*>, #query_name<'a, C, #query_row_struct, #nb_params>, C> for #stmt_name {
                        fn params(&'a mut self, client: &'a #client_mut C, params: &'a #param_name<#lifetime #(#traits_idx3),*>) -> #query_name<'a, C, #query_row_struct, #nb_params> {
                            self.bind(client, #(&params.#params_name),*)
                        }
                    }
                );
            } else {
                let (send_sync, pre_ty, post_ty_lf, pre, post) = if is_async {
                    (
                        "+ Send + Sync",
                        "std::pin::Pin<Box<dyn futures::Future<Output = Result",
                        "> + Send + 'a>>",
                        "Box::pin(self",
                        ")",
                    )
                } else {
                    ("", "Result", "", "self", "")
                };
                quote!(w =>
                    impl <'a, C: GenericClient #send_sync, #(#traits_idx: #traits),*> #client::Params<'a, #param_name<#lifetime #(#traits_idx2),*>, #pre_ty<u64, #backend::Error>#post_ty_lf, C> for #stmt_name {
                        fn params(&'a mut self, client: &'a #client_mut C, params: &'a #param_name<#lifetime #(#traits_idx3),*>) -> #pre_ty<u64, #backend::Error>#post_ty_lf {
                            #pre.bind(client, #(&params.#params_name),*)#post
                        }
                    }
                );
            }
        }
    }
}

/// Generates type definitions for custom user types. This includes domains, composites and enums.
/// If the type is not `Copy`, then a Borrowed version will be generated.
fn gen_custom_type(
    w: &mut impl Write,
    schema: &str,
    prepared: &PreparedType,
    CodegenSettings {
        derive_ser,
        is_async,
    }: CodegenSettings,
) {
    let PreparedType {
        struct_name,
        content,
        is_copy,
        is_params,
        name,
    } = prepared;
    let copy = if *is_copy { "Copy," } else { "" };
    let ser_str = if derive_ser { "serde::Serialize," } else { "" };
    let name_str = format!("\"{name}\"");
    match content {
        PreparedContent::Enum(variants) => {
            quote!(w =>
                #[derive(#ser_str Debug, postgres_types::ToSql, postgres_types::FromSql, Clone, Copy, PartialEq, Eq)]
                #[postgres(name = #name_str)]
                pub enum #struct_name {
                    #(#variants),*
                }
            );
        }
        PreparedContent::Composite(fields) => {
            let fields_name = fields.iter().map(|p| &p.name);
            {
                let fields_name = fields_name.clone();
                let fields_ty = fields.iter().map(|p| p.own_struct());
                quote!(w =>
                    #[derive(#ser_str Debug,postgres_types::FromSql,#copy Clone, PartialEq)]
                    #[postgres(name = #name_str)]
                    pub struct #struct_name {
                        #(pub #fields_name: #fields_ty),*
                    }
                );
            }
            if *is_copy {
                struct_tosql(w, struct_name, fields, name, false, *is_params, is_async);
            } else {
                let fields_owning = fields.iter().map(|p| p.owning_assign());
                let fields_name2 = fields_name.clone();
                let fields_name3 = fields_name.clone();
                let fields_brw = fields.iter().map(|p| p.brw_ty(true, is_async));
                let brw_name = format!("{struct_name}Borrowed");
                quote!(w =>
                    #[derive(Debug)]
                    pub struct #brw_name<'a> {
                        #(pub #fields_name2: #fields_brw),*
                    }
                    impl<'a> From<#brw_name<'a>> for #struct_name {
                        fn from(
                            #brw_name {
                            #(#fields_name3),*
                            }: #brw_name<'a>,
                        ) -> Self {
                            Self {
                                #(#fields_owning),*
                            }
                        }
                    }
                );
                composite_fromsql(w, struct_name, fields, name, schema);
                if !is_params {
                    let param_name = format!("{struct_name}Params");
                    let fields_ty = fields.iter().map(|p| p.param_ty(is_async));
                    let derive = if *is_copy { ",Copy,Clone" } else { "" };
                    quote!(w =>
                        #[derive(Debug #derive)]
                        pub struct #param_name<'a> {
                            #(pub #fields_name: #fields_ty),*
                        }
                    );
                }
                struct_tosql(w, struct_name, fields, name, true, *is_params, is_async);
            }
        }
    }
}

fn gen_type_modules(
    w: &mut impl Write,
    prepared: &IndexMap<String, Vec<PreparedType>>,
    settings: CodegenSettings,
) {
    let modules = prepared.iter().map(|(schema, types)| {
        Lazy::new(move |w| {
            let lazy = Lazy::new(|w| {
                for ty in types {
                    gen_custom_type(w, schema, ty, settings)
                }
            });

            quote!(w =>
            pub mod #schema {
                #lazy
            });
        })
    });
    quote!(w =>
        #[allow(clippy::all, clippy::pedantic)]
        #[allow(unused_variables)]
        #[allow(unused_imports)]
        #[allow(dead_code)]
        pub mod types {
            #(#modules)*
        }
    );
}

pub(crate) fn generate(preparation: Preparation, settings: CodegenSettings) -> String {
    let import = if settings.is_async {
        "use futures::{{StreamExt, TryStreamExt}};use futures; use cornucopia_async::GenericClient;"
    } else {
        "use postgres::{{fallible_iterator::FallibleIterator,GenericClient}};"
    };
    let mut buff = "// This file was generated with `cornucopia`. Do not modify.\n\n".to_string();
    let w = &mut buff;
    // Generate database type
    gen_type_modules(w, &preparation.types, settings);
    // Generate queries
    let query_modules = preparation.modules.iter().map(|module| {
        Lazy::new(move |w| {
            let name = &module.info.name;
            let params_string = module
                .params
                .values()
                .map(|params| Lazy::new(|w| gen_params_struct(w, params, settings)));
            let rows_string = module
                .rows
                .values()
                .map(|row| Lazy::new(|w| gen_row_structs(w, row, settings)));
            let queries_string = module
                .queries
                .values()
                .map(|query| Lazy::new(|w| gen_query_fn(w, &module, query, settings)));
            quote!(w =>
                pub mod #name {
                    #import
                    #(#params_string)*
                    #(#rows_string)*
                    #(#queries_string)*
                }
            );
        })
    });
    quote!(w =>
        #[allow(clippy::all, clippy::pedantic)]
        #[allow(unused_variables)]
        #[allow(unused_imports)]
        #[allow(dead_code)]
        pub mod queries {
            #(#query_modules)*
        }
    );
    buff
}
