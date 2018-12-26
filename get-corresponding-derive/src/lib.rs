// Copyright 2017-2018 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! Custom derive for GetCorresponding.  See
//! `navitia_model::relations` for the documentation.

#![recursion_limit = "128"]

extern crate proc_macro;
use quote::*;
use syn;

use proc_macro::TokenStream;
use std::collections::{HashMap, HashSet};

/// Generation of the `GetCorresponding` trait implementation.
#[proc_macro_derive(GetCorresponding, attributes(get_corresponding))]
pub fn get_corresponding(input: TokenStream) -> TokenStream {
    let s = input.to_string();
    let ast = syn::parse_derive_input(&s).unwrap();
    let gen = impl_get_corresponding(&ast);
    gen.parse().unwrap()
}

fn impl_get_corresponding(ast: &syn::DeriveInput) -> quote::Tokens {
    if let syn::Body::Struct(syn::VariantData::Struct(ref fields)) = ast.body {
        let name = &ast.ident;
        let edges: Vec<_> = fields.iter().filter_map(to_edge).collect();
        let next = floyd_warshall(&edges);
        let edge_to_impl = make_edge_to_get_corresponding(name, &edges);
        let edges_impls = next.iter().map(|(&(from, to), &node)| {
            if from == to {
                quote! {
                    impl GetCorresponding<#to> for IdxSet<#from> {
                        fn get_corresponding(&self, _: &#name) -> IdxSet<#to> {
                            self.clone()
                        }
                    }
                }
            } else if to == node {
                edge_to_impl[&(from, to)].clone()
            } else {
                quote! {
                    impl GetCorresponding<#to> for IdxSet<#from> {
                        fn get_corresponding(&self, pt_objects: &#name) -> IdxSet<#to> {
                            let tmp: IdxSet<#node> = self.get_corresponding(pt_objects);
                            tmp.get_corresponding(pt_objects)
                        }
                    }
                }
            }
        });
        quote! {
            /// A trait that returns a set of objects corresponding to
            /// a given type.
            pub trait GetCorresponding<T: Sized> {
                /// For the given self, returns the set of
                /// corresponding `T` indices.
                fn get_corresponding(&self, model: &#name) -> IdxSet<T>;
            }
            impl #name {
                /// Returns the set of `U` indices corresponding to the `from` set.
                pub fn get_corresponding<T, U>(&self, from: &IdxSet<T>) -> IdxSet<U>
                where
                    IdxSet<T>: GetCorresponding<U>
                {
                    from.get_corresponding(self)
                }
                /// Returns the set of `U` indices corresponding to the `from` index.
                pub fn get_corresponding_from_idx<T, U>(&self, from: Idx<T>) -> IdxSet<U>
                where
                    IdxSet<T>: GetCorresponding<U>
                {
                    self.get_corresponding(&Some(from).into_iter().collect())
                }
            }
            #(#edges_impls)*
        }
    } else {
        quote!()
    }
}

fn to_edge(field: &syn::Field) -> Option<Edge> {
    use syn::MetaItem::*;
    use syn::NestedMetaItem::MetaItem;
    use syn::PathParameters::AngleBracketed;

    let ident = field.ident.as_ref()?.as_ref();
    let mut split = ident.split("_to_");
    let _from_collection = split.next()?;
    let _to_collection = split.next()?;
    if !split.next().is_none() {
        return None;
    }
    let segment = if let syn::Ty::Path(_, ref path) = field.ty {
        path.segments.last()
    } else {
        None
    }?;
    let (from_ty, to_ty) = if let AngleBracketed(ref data) = segment.parameters {
        match (data.types.get(0), data.types.get(1), data.types.get(2)) {
            (Some(from_ty), Some(to_ty), None) => Some((from_ty, to_ty)),
            _ => None,
        }
    } else {
        None
    }?;
    let weight = field
        .attrs
        .iter()
        .flat_map(|attr| match attr.value {
            List(ref i, ref v) if i == "get_corresponding" => v.as_slice(),
            _ => &[],
        })
        .map(|mi| match *mi {
            MetaItem(NameValue(ref i, syn::Lit::Str(ref l, _))) => {
                assert_eq!(i, "weight", "{} is not a valid attribute", i);
                l.parse::<f64>()
                    .expect("`weight` attribute must be convertible to f64")
            }
            _ => panic!("Only `key = \"value\"` attributes supported."),
        })
        .last()
        .unwrap_or(1.);

    Edge {
        ident: ident.into(),
        from: from_ty.clone(),
        to: to_ty.clone(),
        weight: weight,
    }
    .into()
}

fn make_edge_to_get_corresponding<'a>(
    name: &syn::Ident,
    edges: &'a [Edge],
) -> HashMap<(&'a syn::Ty, &'a syn::Ty), quote::Tokens> {
    let mut res = HashMap::default();
    for e in edges {
        let ident: quote::Ident = e.ident.as_str().into();
        let from = &e.from;
        let to = &e.to;
        res.insert(
            (from, to),
            quote! {
                impl GetCorresponding<#to> for IdxSet<#from> {
                    fn get_corresponding(&self, pt_objects: &#name) -> IdxSet<#to> {
                        pt_objects.#ident.get_corresponding_forward(self)
                    }
                }
            },
        );
        res.insert(
            (to, from),
            quote! {
                impl GetCorresponding<#from> for IdxSet<#to> {
                    fn get_corresponding(&self, pt_objects: &#name) -> IdxSet<#from> {
                        pt_objects.#ident.get_corresponding_backward(self)
                    }
                }
            },
        );
    }
    res
}

fn floyd_warshall<'a>(edges: &'a [Edge]) -> HashMap<(&'a Node, &'a Node), &'a Node> {
    use std::f64::INFINITY;
    let mut v = HashSet::<&Node>::default();
    let mut dist = HashMap::<(&Node, &Node), f64>::default();
    let mut next = HashMap::default();
    for e in edges {
        let from = &e.from;
        let to = &e.to;
        v.insert(from);
        v.insert(to);
        dist.insert((from, to), e.weight);
        dist.insert((to, from), e.weight);
        next.insert((from, to), to);
        next.insert((to, from), from);
    }
    for &k in &v {
        for &i in &v {
            let dist_ik = match dist.get(&(i, k)) {
                Some(d) => *d,
                None => continue,
            };
            for &j in &v {
                let dist_kj = match dist.get(&(k, j)) {
                    Some(d) => *d,
                    None => continue,
                };
                let dist_ij = dist.entry((i, j)).or_insert(INFINITY);
                if *dist_ij > dist_ik + dist_kj {
                    *dist_ij = dist_ik + dist_kj;
                    let next_ik = next[&(i, k)];
                    next.insert((i, j), next_ik);
                }
            }
        }
    }
    next
}

struct Edge {
    ident: String,
    from: Node,
    to: Node,
    weight: f64,
}

type Node = syn::Ty;
