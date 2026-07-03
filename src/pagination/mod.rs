//! Pagination result types (`Page<T>`, `CursorPage<T>`) and an RFC 8288
//! `Link` header builder.
//!
//! Not tied to the model layer or any feature flag — build one by hand if
//! your data source isn't [`crate::model::QueryBuilder`] (an external API, an
//! in-memory `Vec`, ...). `QueryBuilder::paginate()` / `::paginate_after()`
//! (require a `model-*` feature) are the batteries-included way to get one
//! directly from a database query.
//!
//! # Offset pagination
//!
//! ```rust
//! use rust_web_server::pagination::Page;
//!
//! let page = Page::new(vec!["a", "b", "c"], 1, 10, 25);
//! assert_eq!(3, page.total_pages);
//! assert!(page.has_next());
//! assert!(!page.has_prev());
//!
//! let link = page.link_header("https://api.example.com/items").unwrap();
//! assert!(link.contains(r#"<https://api.example.com/items?page=2&per_page=10>; rel="next""#));
//! assert!(link.contains(r#"rel="last""#));
//! ```
//!
//! # Cursor (keyset) pagination
//!
//! ```rust
//! use rust_web_server::pagination::CursorPage;
//!
//! let page = CursorPage { items: vec!["a", "b"], next_cursor: Some("42".to_string()) };
//! assert!(page.has_next());
//! let link = page.link_header("https://api.example.com/items", "cursor").unwrap();
//! assert_eq!(r#"<https://api.example.com/items?cursor=42>; rel="next""#, link);
//! ```

#[cfg(test)]
mod tests;

/// A single page of offset-paginated results (`LIMIT`/`OFFSET`-style).
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// 1-based page number.
    pub page: u64,
    pub per_page: u64,
    pub total_items: u64,
    /// `0` when `total_items` is `0`; otherwise `ceil(total_items / per_page)`.
    pub total_pages: u64,
}

impl<T> Page<T> {
    /// Builds a page, computing `total_pages` from `total_items` and `per_page`.
    /// `page` and `per_page` are clamped to a minimum of `1`.
    pub fn new(items: Vec<T>, page: u64, per_page: u64, total_items: u64) -> Self {
        let page = page.max(1);
        let per_page = per_page.max(1);
        let total_pages = if total_items == 0 { 0 } else { (total_items + per_page - 1) / per_page };
        Page { items, page, per_page, total_items, total_pages }
    }

    /// `true` if `page` is before `total_pages`.
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }

    /// `true` if `page` is after `1`.
    pub fn has_prev(&self) -> bool {
        self.page > 1 && self.total_pages > 0
    }

    pub fn next_page(&self) -> Option<u64> {
        self.has_next().then_some(self.page + 1)
    }

    pub fn prev_page(&self) -> Option<u64> {
        self.has_prev().then_some(self.page - 1)
    }

    /// Maps `items` through `f`, leaving all pagination metadata unchanged —
    /// e.g. to turn a `Page<UserRow>` into a `Page<UserDto>` before serializing.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(|item| f(item)).collect(),
            page: self.page,
            per_page: self.per_page,
            total_items: self.total_items,
            total_pages: self.total_pages,
        }
    }

    /// Builds an RFC 8288 `Link` header value with `rel="first"`, `"prev"`,
    /// `"next"`, and `"last"` entries as applicable (a first page omits
    /// `first`/`prev`; a last page omits `next`/`last`). `page`/`per_page`
    /// query parameters are added to (or overwritten on) `base_url`, and any
    /// other existing query parameters are preserved.
    ///
    /// Returns `None` if `base_url` fails to parse, or if there is nothing to
    /// link to (a single page with no prev/next).
    pub fn link_header(&self, base_url: &str) -> Option<String> {
        let mut links = Vec::new();

        if self.has_prev() {
            if let Some(url) = with_query_params(base_url, &[("page", "1"), ("per_page", &self.per_page.to_string())]) {
                links.push(format!(r#"<{}>; rel="first""#, url));
            }
            if let Some(url) = with_query_params(base_url, &[("page", &(self.page - 1).to_string()), ("per_page", &self.per_page.to_string())]) {
                links.push(format!(r#"<{}>; rel="prev""#, url));
            }
        }
        if self.has_next() {
            if let Some(url) = with_query_params(base_url, &[("page", &(self.page + 1).to_string()), ("per_page", &self.per_page.to_string())]) {
                links.push(format!(r#"<{}>; rel="next""#, url));
            }
            if let Some(url) = with_query_params(base_url, &[("page", &self.total_pages.to_string()), ("per_page", &self.per_page.to_string())]) {
                links.push(format!(r#"<{}>; rel="last""#, url));
            }
        }

        if links.is_empty() { None } else { Some(links.join(", ")) }
    }
}

/// A single page of cursor (keyset) paginated results.
///
/// Unlike [`Page`], there is no `total_items`/`total_pages` — computing those
/// would require a separate `COUNT(*)` query, which is exactly what keyset
/// pagination avoids. All you get (and all you need to fetch the next page)
/// is `next_cursor`.
#[derive(Debug, Clone, PartialEq)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    /// Opaque cursor to pass back for the next page. `None` means this is the last page.
    pub next_cursor: Option<String>,
}

impl<T> CursorPage<T> {
    pub fn has_next(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Maps `items` through `f`, leaving `next_cursor` unchanged.
    pub fn map<U>(self, mut f: impl FnMut(T) -> U) -> CursorPage<U> {
        CursorPage {
            items: self.items.into_iter().map(|item| f(item)).collect(),
            next_cursor: self.next_cursor,
        }
    }

    /// Builds a `Link` header with a single `rel="next"` entry, adding (or
    /// overwriting) `cursor_param` as a query parameter on `base_url`.
    /// Returns `None` if there is no next page, or if `base_url` fails to parse.
    pub fn link_header(&self, base_url: &str, cursor_param: &str) -> Option<String> {
        let cursor = self.next_cursor.as_ref()?;
        let url = with_query_params(base_url, &[(cursor_param, cursor.as_str())])?;
        Some(format!(r#"<{}>; rel="next""#, url))
    }
}

fn with_query_params(base_url: &str, params: &[(&str, &str)]) -> Option<String> {
    let mut components = crate::url::URL::parse(base_url).ok()?;
    let mut query = components.query.take().unwrap_or_default();
    for (key, value) in params {
        query.insert(key.to_string(), value.to_string());
    }
    components.query = Some(query);
    crate::url::URL::build(components).ok()
}
