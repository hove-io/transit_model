extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use std::collections::{BTreeMap, BTreeSet};

#[proc_macro_derive(GetCorresponding)]
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
        println!("{:?}", next);
        let edge_to_impl = make_edge_to_get_corresponding(name, &edges);
        let edges_impls = next.iter().filter_map(|(&(from, to), node)| {
            if from == to {
                return None;
            }
            if let Some(token) = edge_to_impl.get(&(from, to)) {
                Some(token.clone())
            } else {
                let from_ty: quote::Ident = from.into();
                let to_ty: quote::Ident = to.into();
                let node_ty: quote::Ident = (*node).into();
                Some(quote! {
                    impl GetCorresponding<#to_ty> for IdxSet<#from_ty> {
                        fn get_corresponding(&self, pt_objects: &PtObjects) -> IdxSet<#to_ty> {
                            let tmp: IdxSet<#node_ty> = self.get_corresponding(pt_objects);
                            tmp.get_corresponding(pt_objects)
                        }
                    }
                })
            }
        });
        quote!(#(#edges_impls)*)
    } else {
        quote!()
    }
}

fn to_edge(field: &syn::Field) -> Option<Edge> {
    let ident = field.ident.as_ref()?;
    let ident = ident.as_ref();
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
    let (from_ty, to_ty) = if let syn::PathParameters::AngleBracketed(ref data) = segment.parameters
    {
        match (data.types.get(0), data.types.get(1), data.types.get(2)) {
            (Some(from_ty), Some(to_ty), None) => Some((from_ty, to_ty)),
            _ => None,
        }
    } else {
        None
    }?;
    let from_ty = get_ident_from_ty(from_ty)?;
    let to_ty = get_ident_from_ty(to_ty)?;
    Edge {
        ident: ident.into(),
        from: from_ty.into(),
        to: to_ty.into(),
    }.into()
}

fn get_ident_from_ty(ty: &syn::Ty) -> Option<&str> {
    match *ty {
        syn::Ty::Path(_, ref path) => path.segments.last().map(|segment| segment.ident.as_ref()),
        _ => None,
    }
}

fn make_edge_to_get_corresponding<'a>(
    name: &syn::Ident,
    edges: &'a [Edge],
) -> BTreeMap<(&'a str, &'a str), quote::Tokens> {
    let mut res = BTreeMap::default();
    for e in edges {
        let ident: quote::Ident = e.ident.as_str().into();
        let from = e.from.as_str();
        let from_ty: quote::Ident = from.into();
        let to = e.to.as_str();
        let to_ty: quote::Ident = to.into();
        res.insert(
            (from, to),
            quote! {
                impl GetCorresponding<#to_ty> for IdxSet<#from_ty> {
                    fn get_corresponding(&self, pt_objects: &#name) -> IdxSet<#to_ty> {
                        pt_objects.#ident.get_corresponding_forward(self)
                    }
                }
            },
        );
        res.insert(
            (to, from),
            quote! {
                impl GetCorresponding<#from_ty> for IdxSet<#to_ty> {
                    fn get_corresponding(&self, pt_objects: &#name) -> IdxSet<#from_ty> {
                        pt_objects.#ident.get_corresponding_backward(self)
                    }
                }
            },
        );
    }
    res
}

fn floyd_warshall<'a>(edges: &'a [Edge]) -> BTreeMap<(&'a str, &'a str), &'a str> {
    use std::f64::INFINITY;
    let mut v = BTreeSet::<&str>::default();
    let mut dist = BTreeMap::<(&str, &str), f64>::default();
    let mut next = BTreeMap::default();
    for e in edges {
        let from = e.from.as_str();
        let to = e.to.as_str();
        v.insert(from);
        v.insert(to);
        dist.insert((from, to), 1.);
        dist.insert((to, from), 1.);
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
}

type Node = String;
