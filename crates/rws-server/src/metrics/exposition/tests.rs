use super::*;

/// A minimal protobuf reader: (field, varint value or bytes).
#[derive(Debug)]
enum Field {
    Varint(u64),
    Fixed64(u64),
    Bytes(Vec<u8>),
}

fn read_varint(buf: &[u8], pos: &mut usize) -> u64 {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let b = buf[*pos];
        *pos += 1;
        v |= ((b & 0x7f) as u64) << shift;
        if b < 0x80 {
            return v;
        }
        shift += 7;
    }
}

fn fields(buf: &[u8]) -> Vec<(u32, Field)> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < buf.len() {
        let key = read_varint(buf, &mut pos);
        let field = (key >> 3) as u32;
        let value = match key & 7 {
            0 => Field::Varint(read_varint(buf, &mut pos)),
            1 => {
                let v = u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
                pos += 8;
                Field::Fixed64(v)
            }
            2 => {
                let len = read_varint(buf, &mut pos) as usize;
                pos += len;
                Field::Bytes(buf[pos - len..pos].to_vec())
            }
            wire => panic!("wire type {wire}"),
        };
        out.push((field, value));
    }
    out
}

fn delimited(buf: &[u8]) -> Vec<Vec<u8>> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < buf.len() {
        let len = read_varint(buf, &mut pos) as usize;
        out.push(buf[pos..pos + len].to_vec());
        pos += len;
    }
    out
}

fn get<'a>(fields: &'a [(u32, Field)], n: u32) -> Vec<&'a Field> {
    fields.iter().filter(|(f, _)| *f == n).map(|(_, v)| v).collect()
}

fn bytes(field: &Field) -> &[u8] {
    match field {
        Field::Bytes(b) => b,
        other => panic!("not bytes: {other:?}"),
    }
}

fn zigzag_decode(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

#[test]
fn prometheus_accept_headers_get_protobuf() {
    // What Prometheus 3 sends with scrape_native_histograms.
    assert!(accepts_protobuf("application/vnd.google.protobuf;proto=io.prometheus.client.MetricFamily;encoding=delimited;q=0.6,application/openmetrics-text;version=1.0.0;q=0.5,text/plain;version=0.0.4;q=0.3,*/*;q=0.1"));
    assert!(accepts_protobuf("application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily; encoding=delimited"));
    assert!(!accepts_protobuf("text/plain;version=0.0.4"));
    assert!(!accepts_protobuf("*/*"));
    assert!(!accepts_protobuf("application/vnd.google.protobuf;proto=io.prometheus.client.MetricFamily;q=0"));
    assert!(!accepts_protobuf("application/vnd.google.protobuf;proto=other.Message"));
}

#[test]
fn text_is_parsed_into_families() {
    let text = "# HELP a_total Things\n# TYPE a_total counter\na_total 3\n\n\
        # HELP g Gauge\n# TYPE g gauge\ng{x=\"q\\\"uote\",y=\"z\"} -1.5\n\
        # HELP h Hist\n# TYPE h histogram\n\
        h_bucket{r=\"1\",le=\"0.1\"} 1\nh_bucket{r=\"1\",le=\"+Inf\"} 2\nh_sum{r=\"1\"} 0.7\nh_count{r=\"1\"} 2\n";
    let families = parse_text(text);
    assert_eq!(3, families.len());
    assert_eq!((Kind::Counter, Value::Number(3.0)), (families[0].kind, families[0].samples[0].value.clone()));
    assert_eq!(vec![("x".to_string(), "q\"uote".to_string()), ("y".to_string(), "z".to_string())], families[1].samples[0].labels);
    let Value::Histogram(classic, None) = &families[2].samples[0].value else { panic!("{families:?}") };
    assert_eq!(ClassicHistogram { count: 2, sum: 0.7, buckets: vec![(0.1, 1)] }, *classic);
    assert_eq!(vec![("r".to_string(), "1".to_string())], families[2].samples[0].labels);
}

#[test]
fn garbage_lines_are_skipped() {
    let families = parse_text("x{a=\"unterminated 1\ny{ 2\nz notanumber\nok 1\n");
    assert_eq!(vec!["ok"], families.iter().map(|f| f.name.as_str()).collect::<Vec<_>>());
}

#[test]
fn the_encoding_follows_metrics_proto() {
    let mut native = http::Histogram::default();
    native.observe(1.0);
    native.observe(2.0);
    let families = vec![
        Family { name: "c_total".into(), help: "C".into(), kind: Kind::Counter, samples: vec![Sample { labels: vec![("k".into(), "v".into())], value: Value::Number(7.0) }] },
        Family {
            name: "d".into(),
            help: String::new(),
            kind: Kind::Histogram,
            samples: vec![Sample { labels: vec![], value: Value::Histogram(ClassicHistogram { count: 2, sum: 3.0, buckets: vec![(1.0, 1)] }, Some(native)) }],
        },
    ];
    let messages = delimited(&encode(&families));
    assert_eq!(2, messages.len());

    let counter = fields(&messages[0]);
    assert_eq!(b"c_total", bytes(get(&counter, 1)[0]));
    assert!(matches!(get(&counter, 3)[0], Field::Varint(0)), "COUNTER");
    let metric = fields(bytes(get(&counter, 4)[0]));
    let label = fields(bytes(get(&metric, 1)[0]));
    assert_eq!((b"k".as_slice(), b"v".as_slice()), (bytes(get(&label, 1)[0]), bytes(get(&label, 2)[0])));
    let value = fields(bytes(get(&metric, 3)[0]));
    assert!(matches!(get(&value, 1)[0], Field::Fixed64(v) if f64::from_bits(*v) == 7.0));

    let histogram_family = fields(&messages[1]);
    assert!(get(&histogram_family, 2).is_empty(), "no empty help");
    assert!(matches!(get(&histogram_family, 3)[0], Field::Varint(4)), "HISTOGRAM");
    let h = fields(bytes(get(&fields(bytes(get(&histogram_family, 4)[0])), 7)[0]));
    assert!(matches!(get(&h, 1)[0], Field::Varint(2)));
    let bucket = fields(bytes(get(&h, 3)[0]));
    assert!(matches!(get(&bucket, 1)[0], Field::Varint(1)));
    assert!(matches!(get(&h, 5)[0], Field::Varint(v) if zigzag_decode(*v) == 3), "schema 3");
    // Buckets 0 and 8: two spans, (0, 1) and (7, 1).
    let spans: Vec<(i64, u64)> = get(&h, 12)
        .iter()
        .map(|s| {
            let s = fields(bytes(s));
            let offset = match get(&s, 1).first() { Some(Field::Varint(v)) => zigzag_decode(*v), _ => 0 };
            let length = match get(&s, 2).first() { Some(Field::Varint(v)) => *v, _ => 0 };
            (offset, length)
        })
        .collect();
    assert_eq!(vec![(0, 1), (7, 1)], spans);
    let packed = bytes(get(&h, 13)[0]);
    let mut pos = 0;
    let deltas: Vec<i64> = std::iter::from_fn(|| (pos < packed.len()).then(|| zigzag_decode(read_varint(packed, &mut pos)))).collect();
    assert_eq!(vec![1, 0], deltas);
}

#[test]
fn every_metric_is_in_the_protobuf_output() {
    http::record(http::Series::new("GET", 299, false, "HTTP/1.1"), 0.01);
    let messages = delimited(&protobuf());
    let names: Vec<String> = messages.iter().map(|m| String::from_utf8(bytes(get(&fields(m), 1)[0]).to_vec()).unwrap()).collect();
    assert!(names.contains(&"rws_requests_total".to_string()), "{names:?}");
    assert!(names.contains(&http::NAME.to_string()), "{names:?}");
}
