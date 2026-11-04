//! [`Path<T>`]: route parameters as typed values.

use std::fmt::Display;
use std::str::FromStr;

use super::{rejection, FromRequestParts, RequestParts};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::router::PathParams;

/// The route's parameters, parsed: `Path(id): Path<u64>` for one,
/// `Path((id, slug)): Path<(u64, String)>` for several, or a
/// `#[derive(FromPath)]` struct whose fields are named after them.
///
/// Values are percent-decoded before parsing (`/tags/a%20b` → `"a b"`;
/// [`PathParams`] keeps them raw). A value that doesn't parse answers
/// `400 Bad Request`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<T>(pub T);

impl<T> std::ops::Deref for Path<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<S, T: FromPath> FromRequestParts<S> for Path<T> {
    fn from_request_parts(parts: &RequestParts<'_, S>) -> Result<Self, Response> {
        T::from_path(parts.params, parts.path_names)
            .map(Path)
            .map_err(|detail| rejection(parts.request, STATUS_CODE_REASON_PHRASE.n400_bad_request, &detail))
    }
}

/// A type [`Path`] can build from route parameters.
///
/// Implemented for the standard scalars (one parameter), tuples of up to
/// six `FromStr` values (one parameter each, in order), and structs with
/// `#[derive(FromPath)]` (one parameter per field, by name). A type of your
/// own with `FromStr` works as a one-tuple: `Path((slug,)): Path<(Slug,)>`.
pub trait FromPath: Sized {
    /// How many parameters this type takes.
    const ARITY: usize;
    /// The parameters by name (a struct's fields), or empty when they are
    /// taken in order. The route macros check these against the pattern.
    const NAMES: &'static [&'static str] = &[];
    /// Build the value. `names` are the route parameters to take, in order
    /// (`ARITY` of them); an error is the rejection's detail.
    fn from_path(params: &PathParams, names: &[&str]) -> Result<Self, String>;
}

/// One parameter, percent-decoded and parsed.
pub fn parse_param<T>(params: &PathParams, name: &str) -> Result<T, String>
where
    T: FromStr,
    T::Err: Display,
{
    let raw = params.get(name).ok_or_else(|| format!("the route has no path parameter `{name}`"))?;
    let decoded = percent_decode(raw);
    decoded.parse::<T>().map_err(|e| format!("path parameter `{name}`: {e}"))
}

/// `%HH` as a byte, everything else as it is (`+` stays a plus); invalid
/// UTF-8 becomes U+FFFD.
fn percent_decode(raw: &str) -> String {
    if !raw.contains('%') {
        return raw.to_string();
    }
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |c: u8| (c as char).to_digit(16);
    while i < b.len() {
        if b[i] == b'%' {
            if let (Some(h), Some(l)) = (b.get(i + 1).and_then(|c| hex(*c)), b.get(i + 2).and_then(|c| hex(*c))) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `T` takes exactly the route parameters `route` — evaluated in a
/// `const` by the route macros, so a mismatch is a compile error.
pub const fn path_fits<T: FromPath>(route: &[&str]) -> bool {
    if T::NAMES.is_empty() {
        return T::ARITY == route.len();
    }
    if T::NAMES.len() != route.len() {
        return false;
    }
    let mut i = 0;
    while i < T::NAMES.len() {
        if !contains(route, T::NAMES[i]) {
            return false;
        }
        i += 1;
    }
    true
}

const fn contains(list: &[&str], name: &str) -> bool {
    let mut i = 0;
    while i < list.len() {
        if str_eq(list[i], name) {
            return true;
        }
        i += 1;
    }
    false
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn one<T>(params: &PathParams, names: &[&str]) -> Result<T, String>
where
    T: FromStr,
    T::Err: Display,
{
    match names {
        [name] => parse_param(params, name),
        _ => Err(format!("expected 1 path parameter, the route passes {}", names.len())),
    }
}

macro_rules! scalar_from_path {
    ($($t:ty),*) => {$(
        impl FromPath for $t {
            const ARITY: usize = 1;
            fn from_path(params: &PathParams, names: &[&str]) -> Result<Self, String> {
                one(params, names)
            }
        }
    )*};
}

scalar_from_path!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, bool, char, String,
    std::net::IpAddr, std::net::Ipv4Addr, std::net::Ipv6Addr
);

macro_rules! tuple_from_path {
    ($n:expr; $($t:ident $i:tt),+) => {
        impl<$($t),+> FromPath for ($($t,)+)
        where $($t: FromStr, $t::Err: Display),+
        {
            const ARITY: usize = $n;
            fn from_path(params: &PathParams, names: &[&str]) -> Result<Self, String> {
                if names.len() != $n {
                    return Err(format!("expected {} path parameters, the route passes {}", $n, names.len()));
                }
                Ok(($(parse_param::<$t>(params, names[$i])?,)+))
            }
        }
    };
}

tuple_from_path!(1; A 0);
tuple_from_path!(2; A 0, B 1);
tuple_from_path!(3; A 0, B 1, C 2);
tuple_from_path!(4; A 0, B 1, C 2, D 3);
tuple_from_path!(5; A 0, B 1, C 2, D 3, E 4);
tuple_from_path!(6; A 0, B 1, C 2, D 3, E 4, F 5);
