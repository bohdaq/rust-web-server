//! [`Query<T>`] and [`Form<T>`]: `application/x-www-form-urlencoded` pairs
//! deserialized into a struct (`serde`), with a small deserializer of its
//! own (no `serde_urlencoded`).

use serde::de::{self, DeserializeOwned, IntoDeserializer, Visitor};
use serde::de::value::Error;

use super::{rejection, FromRequestParts, RequestParts};
use crate::response::{Response, STATUS_CODE_REASON_PHRASE};
use crate::url::URL;

/// The query string deserialized into `T`: `Query(page): Query<Page>`.
///
/// Fields are scalars, `String`s, `Option`s (absent or empty → `None`),
/// unit enums, or `Vec`s of them (`?tag=a&tag=b`). A query that doesn't
/// fit `T` answers `400 Bad Request`. The untyped map is
/// [`extract::Query`](crate::extract::Query).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query<T>(pub T);

impl<T> std::ops::Deref for Query<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<S, T: DeserializeOwned> FromRequestParts<S> for Query<T> {
    fn from_request_parts(parts: &RequestParts<'_, S>) -> Result<Self, Response> {
        let uri = &parts.request.request_uri;
        let query = uri.split_once('?').map(|(_, q)| q).unwrap_or("");
        let query = query.split('#').next().unwrap_or("");
        from_urlencoded(query)
            .map(Query)
            .map_err(|e| rejection(parts.request, STATUS_CODE_REASON_PHRASE.n400_bad_request, &format!("query string: {e}")))
    }
}

/// An `application/x-www-form-urlencoded` body deserialized into `T`:
/// `Form(signup): Form<Signup>`. Same field types as [`Query`].
///
/// Rejections: `415` when the `Content-Type` isn't
/// `application/x-www-form-urlencoded`, `400` for a body that isn't UTF-8,
/// `422` when the fields don't fit `T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form<T>(pub T);

impl<T> std::ops::Deref for Form<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

const FORM_TYPE: &str = "application/x-www-form-urlencoded";

impl<S, T: DeserializeOwned> FromRequestParts<S> for Form<T> {
    fn from_request_parts(parts: &RequestParts<'_, S>) -> Result<Self, Response> {
        let request = parts.request;
        let content_type: &str = request.header("Content-Type").map(|h| h.value.as_ref()).unwrap_or("");
        let essence = content_type.split(';').next().unwrap_or("").trim();
        if !essence.eq_ignore_ascii_case(FORM_TYPE) {
            let detail = format!("expected Content-Type {FORM_TYPE}");
            return Err(rejection(request, STATUS_CODE_REASON_PHRASE.n415_unsupported_media_type, &detail));
        }
        let body = std::str::from_utf8(&request.body)
            .map_err(|_| rejection(request, STATUS_CODE_REASON_PHRASE.n400_bad_request, "request body is not valid UTF-8"))?;
        from_urlencoded(body)
            .map(Form)
            .map_err(|e| rejection(request, STATUS_CODE_REASON_PHRASE.n422_unprocessable_entity, &format!("form: {e}")))
    }
}

/// `a=1&b=x+y&b=z` into `T`.
pub(crate) fn from_urlencoded<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    // Keys in first-seen order; the index keeps a body of many distinct
    // keys linear.
    let mut pairs: Vec<(String, Vec<String>)> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for pair in input.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let (k, v) = (URL::form_decode(k), URL::form_decode(v));
        match index.get(&k) {
            Some(&i) => pairs[i].1.push(v),
            None => {
                index.insert(k.clone(), pairs.len());
                pairs.push((k, vec![v]));
            }
        }
    }
    T::deserialize(Pairs(pairs))
}

/// Every shape the deserializer handles, for the fuzzer and the
/// stack-depth test.
#[doc(hidden)]
pub fn fuzz(input: &str) {
    use std::collections::HashMap;
    let _ = from_urlencoded::<HashMap<String, String>>(input);
    let _ = from_urlencoded::<HashMap<String, Vec<String>>>(input);
    let _ = from_urlencoded::<HashMap<String, Option<u64>>>(input);
    let _ = from_urlencoded::<HashMap<String, (bool, char)>>(input);
    let _ = from_urlencoded::<Vec<(String, f64)>>(input);
}

struct Pairs(Vec<(String, Vec<String>)>);

impl<'de> de::Deserializer<'de> for Pairs {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(PairsAccess { pairs: self.0.into_iter(), value: None })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

struct PairsAccess {
    pairs: std::vec::IntoIter<(String, Vec<String>)>,
    value: Option<Vec<String>>,
}

impl<'de> de::MapAccess<'de> for PairsAccess {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Error> {
        match self.pairs.next() {
            Some((key, values)) => {
                self.value = Some(values);
                seed.deserialize(key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let values = self.value.take().unwrap_or_default();
        seed.deserialize(Values(values))
    }
}

/// A field's values (one, or several for a repeated key). Scalars take the
/// last; sequences take them all.
struct Values(Vec<String>);

impl Values {
    fn last(self) -> String {
        self.0.into_iter().next_back().unwrap_or_default()
    }

    fn parse<T: std::str::FromStr>(self, what: &str) -> Result<T, Error>
    where
        T::Err: std::fmt::Display,
    {
        let value = self.last();
        value.parse::<T>().map_err(|e| de::Error::custom(format!("{value:?} is not a valid {what}: {e}")))
    }
}

macro_rules! parse_scalar {
    ($($method:ident => $visit:ident $t:ty),*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            visitor.$visit(self.parse::<$t>(stringify!($t))?)
        }
    )*};
}

impl<'de> de::Deserializer<'de> for Values {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.0.len() > 1 {
            return self.deserialize_seq(visitor);
        }
        visitor.visit_string(self.last())
    }

    parse_scalar! {
        deserialize_bool => visit_bool bool,
        deserialize_i8 => visit_i8 i8, deserialize_i16 => visit_i16 i16,
        deserialize_i32 => visit_i32 i32, deserialize_i64 => visit_i64 i64,
        deserialize_i128 => visit_i128 i128,
        deserialize_u8 => visit_u8 u8, deserialize_u16 => visit_u16 u16,
        deserialize_u32 => visit_u32 u32, deserialize_u64 => visit_u64 u64,
        deserialize_u128 => visit_u128 u128,
        deserialize_f32 => visit_f32 f32, deserialize_f64 => visit_f64 f64,
        deserialize_char => visit_char char
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.last())
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_string(self.last())
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.0.iter().all(|v| v.is_empty()) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_seq(de::value::SeqDeserializer::new(self.0.into_iter().map(|v| Values(vec![v]))))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(self.last().into_deserializer())
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bytes byte_buf unit_struct tuple tuple_struct map struct identifier ignored_any
    }
}

impl<'de> IntoDeserializer<'de, Error> for Values {
    type Deserializer = Values;
    fn into_deserializer(self) -> Values {
        self
    }
}
