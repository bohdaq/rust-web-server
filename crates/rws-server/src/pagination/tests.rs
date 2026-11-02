use super::*;

// ── Page ─────────────────────────────────────────────────────────────────────

#[test]
fn new_computes_total_pages() {
    let page = Page::new(vec![1, 2, 3], 1, 10, 25);
    assert_eq!(3, page.total_pages);
}

#[test]
fn new_computes_exact_total_pages() {
    let page = Page::new(vec![1, 2, 3], 1, 10, 30);
    assert_eq!(3, page.total_pages);
}

#[test]
fn new_zero_total_items_has_zero_pages() {
    let page: Page<i32> = Page::new(vec![], 1, 10, 0);
    assert_eq!(0, page.total_pages);
    assert!(!page.has_next());
    assert!(!page.has_prev());
}

#[test]
fn new_clamps_page_and_per_page_to_minimum_one() {
    let page = Page::new(vec![1], 0, 0, 10);
    assert_eq!(1, page.page);
    assert_eq!(1, page.per_page);
}

#[test]
fn first_page_has_next_but_not_prev() {
    let page = Page::new(vec![1, 2], 1, 10, 25);
    assert!(page.has_next());
    assert!(!page.has_prev());
    assert_eq!(Some(2), page.next_page());
    assert_eq!(None, page.prev_page());
}

#[test]
fn middle_page_has_next_and_prev() {
    let page = Page::new(vec![1, 2], 2, 10, 25);
    assert!(page.has_next());
    assert!(page.has_prev());
    assert_eq!(Some(3), page.next_page());
    assert_eq!(Some(1), page.prev_page());
}

#[test]
fn last_page_has_prev_but_not_next() {
    let page = Page::new(vec![1], 3, 10, 25);
    assert!(!page.has_next());
    assert!(page.has_prev());
    assert_eq!(None, page.next_page());
    assert_eq!(Some(2), page.prev_page());
}

#[test]
fn single_page_has_neither_next_nor_prev() {
    let page = Page::new(vec![1, 2], 1, 10, 2);
    assert!(!page.has_next());
    assert!(!page.has_prev());
}

#[test]
fn map_transforms_items_and_preserves_metadata() {
    let page = Page::new(vec![1, 2, 3], 2, 10, 25);
    let mapped = page.clone().map(|n| n.to_string());
    assert_eq!(vec!["1".to_string(), "2".to_string(), "3".to_string()], mapped.items);
    assert_eq!(page.page, mapped.page);
    assert_eq!(page.per_page, mapped.per_page);
    assert_eq!(page.total_items, mapped.total_items);
    assert_eq!(page.total_pages, mapped.total_pages);
}

#[test]
fn link_header_first_page_omits_first_and_prev() {
    let page = Page::new(vec![1], 1, 10, 25);
    let link = page.link_header("https://api.example.com/items").unwrap();
    assert!(!link.contains(r#"rel="first""#));
    assert!(!link.contains(r#"rel="prev""#));
    assert!(link.contains(r#"rel="next""#));
    assert!(link.contains(r#"rel="last""#));
}

#[test]
fn link_header_last_page_omits_next_and_last() {
    let page = Page::new(vec![1], 3, 10, 25);
    let link = page.link_header("https://api.example.com/items").unwrap();
    assert!(link.contains(r#"rel="first""#));
    assert!(link.contains(r#"rel="prev""#));
    assert!(!link.contains(r#"rel="next""#));
    assert!(!link.contains(r#"rel="last""#));
}

#[test]
fn link_header_middle_page_has_all_four_rels() {
    let page = Page::new(vec![1], 2, 10, 30);
    let link = page.link_header("https://api.example.com/items").unwrap();
    for rel in ["first", "prev", "next", "last"] {
        assert!(link.contains(&format!(r#"rel="{}""#, rel)), "missing rel={} in: {}", rel, link);
    }
}

#[test]
fn link_header_single_page_returns_none() {
    let page = Page::new(vec![1], 1, 10, 5);
    assert_eq!(None, page.link_header("https://api.example.com/items"));
}

#[test]
fn link_header_contains_correct_page_numbers() {
    let page = Page::new(vec![1], 2, 10, 30);
    let link = page.link_header("https://api.example.com/items").unwrap();
    assert!(link.contains("page=1"), "expected first page=1 in: {}", link);
    assert!(link.contains("page=3"), "expected next page=3 in: {}", link);
    assert!(link.contains("per_page=10"), "expected per_page=10 in: {}", link);
}

#[test]
fn link_header_preserves_existing_query_params() {
    let page = Page::new(vec![1], 1, 10, 25);
    let link = page.link_header("https://api.example.com/items?active=true").unwrap();
    assert!(link.contains("active=true"), "expected existing query param preserved in: {}", link);
}

#[test]
fn link_header_invalid_base_url_returns_none() {
    let page = Page::new(vec![1], 1, 10, 25);
    assert_eq!(None, page.link_header("not a url"));
}

// ── CursorPage ───────────────────────────────────────────────────────────────

#[test]
fn cursor_page_has_next_when_cursor_present() {
    let page = CursorPage { items: vec![1, 2], next_cursor: Some("42".to_string()) };
    assert!(page.has_next());
}

#[test]
fn cursor_page_no_next_when_cursor_absent() {
    let page: CursorPage<i32> = CursorPage { items: vec![1, 2], next_cursor: None };
    assert!(!page.has_next());
}

#[test]
fn cursor_page_map_transforms_items_and_preserves_cursor() {
    let page = CursorPage { items: vec![1, 2], next_cursor: Some("42".to_string()) };
    let mapped = page.map(|n| n * 10);
    assert_eq!(vec![10, 20], mapped.items);
    assert_eq!(Some("42".to_string()), mapped.next_cursor);
}

#[test]
fn cursor_page_link_header_with_next_cursor() {
    let page = CursorPage { items: vec![1], next_cursor: Some("42".to_string()) };
    let link = page.link_header("https://api.example.com/items", "cursor").unwrap();
    assert_eq!(r#"<https://api.example.com/items?cursor=42>; rel="next""#, link);
}

#[test]
fn cursor_page_link_header_without_next_cursor_is_none() {
    let page: CursorPage<i32> = CursorPage { items: vec![1], next_cursor: None };
    assert_eq!(None, page.link_header("https://api.example.com/items", "cursor"));
}

#[test]
fn cursor_page_link_header_invalid_base_url_returns_none() {
    let page = CursorPage { items: vec![1], next_cursor: Some("42".to_string()) };
    assert_eq!(None, page.link_header("not a url", "cursor"));
}
