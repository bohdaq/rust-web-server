//! Unit tests for PEM loading (`load_certs` / `load_key` / `load_certified_key`).

use super::{load_certified_key, load_certs, load_key};

// Self-signed `CN=localhost` test fixtures (openssl, 100-year validity). The
// three keys cover every private-key encoding `load_key` must accept.
const CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIC6jCCAdKgAwIBAgIUED2cZxQ9gDDTomwD9VYPyKiAfsUwDQYJKoZIhvcNAQEL\nBQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MCAXDTI2MTAyMzIxNTU1N1oYDzIxMjYw\nOTI5MjE1NTU3WjAUMRIwEAYDVQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEB\nAQUAA4IBDwAwggEKAoIBAQCjzx9njy4zgEeWCNOO53J5pWEMlHy1MIK+WcPX+I2S\ny3nIn4G8WtF5CLkhfPqX3aetOSzFr9fAwmpJE/iUVW3x63aTCMTPRTFSFZgrKqAr\nu3N/mIrK9X+ZSytzovmCOHyWkwwxeFSY74RfLPARFhf6vypzLpovcAdVSgi5nY3O\nUij6eyhk5hCFB8RNwv/1XIWICsRnTEjM2gakGnPZckBQuC8HfymvFLxdFPMo91z+\ng7dpRmtkvAPnvPcwhs6Pt2X6mx1awDMj2WJdCBKz80MBrVEeudD9PyC2YyG8bKaD\nXWDJVPLWiVAXTNin/h65FE+ucmyp75GIojn01cp5Um8nAgMBAAGjMjAwMB0GA1Ud\nDgQWBBSHthoJUSE1wwl28X3FuMq8MhpaFjAPBgNVHRMBAf8EBTADAQH/MA0GCSqG\nSIb3DQEBCwUAA4IBAQCM+q76uq3B0pyoUrefknPJUP60FVvOGF58/EPwJliVb7Vw\nOUQ1ZDzk4+7/YSDaUMpgboYcp6sFg7PAv77iCs6LpQoqG24A7qdKKyNCEFWyCh1e\nCKgyCM9u0RcOQ4oTOvnkNNYkDM9DdTJzV29asbJBf4z4lIMWxGSE6RSN8GnWomOr\nqSkiip2x3IHoOxHnqOZWXAR1jKysBWdXxXgNF1ALZ41c7vxaFLDEKqHkY2z9bGrR\nmNBsYiCojf7MxJoKaYYONCUG/Uu5E+1hJMMgsNO3T+VC/0xJoYqYFsP6TQrrKJ7w\nIEZ/JeVe670tqzCC/9wzBPVDDoIkGfpL5s5RT1vL\n-----END CERTIFICATE-----\n";

const RSA_PKCS8_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEuwIBADANBgkqhkiG9w0BAQEFAASCBKUwggShAgEAAoIBAQCjzx9njy4zgEeW\nCNOO53J5pWEMlHy1MIK+WcPX+I2Sy3nIn4G8WtF5CLkhfPqX3aetOSzFr9fAwmpJ\nE/iUVW3x63aTCMTPRTFSFZgrKqAru3N/mIrK9X+ZSytzovmCOHyWkwwxeFSY74Rf\nLPARFhf6vypzLpovcAdVSgi5nY3OUij6eyhk5hCFB8RNwv/1XIWICsRnTEjM2gak\nGnPZckBQuC8HfymvFLxdFPMo91z+g7dpRmtkvAPnvPcwhs6Pt2X6mx1awDMj2WJd\nCBKz80MBrVEeudD9PyC2YyG8bKaDXWDJVPLWiVAXTNin/h65FE+ucmyp75GIojn0\n1cp5Um8nAgMBAAECgf9XDhgl0Gg6CkFWdHs1XNHis2PMk+9W0eC3FDikqLuA0fel\nN8qM+gW7ew6zttikodJ6zTAe8yAsfvFs637uacCU0OomJlt4aXS4/QKZhTu0eEZm\nJriIhsHmwwWc1+xflxUVdAoATZbpowuinM+jPmllxVNnlH2KkQWjbuLyhzHpX8Yf\nSzpLpho0zvia3vEt8HUik1efwNs4rJ64UQEnMDlwrRUgcPk/ebULQuyy8B60P7cx\nKCkN1aDlsZBgnK8tFvCljVENeEb4y+EKVDfNv1Ijdv2+C3kugg3Cbl2d0fXiRzjo\nqsYer3y5Jf6FzjBlBcVVgquLDrrRSoVWS+gxSfECgYEAziOfP5xWCc06SW/YKSl5\nOZAFyQQvV6aqTtgLJzIomlkR/OLMzq8a9WlDp3bTquYbBkrNE3ADql+W6du1GAS+\nkaDf5SloA/CzwgJzX7beXFcUg71UyPhoDckPXmLMSehhCdESnvcnrMwWsIW+VPLN\nX+QIwyUw4y5EG5k/3oRzxHsCgYEAy25fZmuIbS2lPuO3AVozAkB5CQDYr1bVmd2Y\nvlt3Tb8lW22T3guUG6mHDdIlDLOmbj0ocf+iOPy8MrvuHHQka2DQhHGT0xW46pNp\nINbWqz9cTB9GwqW7RlZ2XK/VEmEGnAurbZpwqI48uCS/wGySwnFZijOfe3IpnbTZ\nWKdzTkUCgYBZSPCW+r4BbNTMLhdwbLhHCGyZDqj6zAOENutNxNn1UJML96hB9XI1\nzawyArPXl+coqIl+28uapeCV1kXv604ngYi4l2CoJD+XzrvuPyS81jLahM0OZxma\nrvVrKlvVv0MYn2fzVpXmsFc7ihJQdQIjgH+TFq7AF7AETDJvzOrftQKBgQC6hsJM\ni860pMS+bFcxkz3wE6OnALKt2sXR5iCWchFrJRegxFFvqFguIPwnTQcRpWu16re6\nBzX8YKhDMJO8XTXDiyrGNcngIyvBUlZzeOmubVM4rPjZotKbRM4UYFjTGEnSLNUL\nky83Lew4v+FeE29hgEMkd/8EVoIYC2m6YxdTrQKBgF5FE5BOaC00RkY8kaE7Hc36\nG/vlu7JSe7F8h4eRtEw0LeLr5WwNn82JLQjbvqzvIbT3/ADhVW79c9GRBJUMFDhh\n971T+4BeMnoRGSFgPABYdA1lJ7gLSYDKrPYRwzUlgz3/RjF6QNALqU5B+Asb5Mu4\n42oB5X34bE9EagYjcHP1\n-----END PRIVATE KEY-----\n";

const RSA_PKCS1_KEY_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----\nMIIEoQIBAAKCAQEAo88fZ48uM4BHlgjTjudyeaVhDJR8tTCCvlnD1/iNkst5yJ+B\nvFrReQi5IXz6l92nrTksxa/XwMJqSRP4lFVt8et2kwjEz0UxUhWYKyqgK7tzf5iK\nyvV/mUsrc6L5gjh8lpMMMXhUmO+EXyzwERYX+r8qcy6aL3AHVUoIuZ2NzlIo+nso\nZOYQhQfETcL/9VyFiArEZ0xIzNoGpBpz2XJAULgvB38prxS8XRTzKPdc/oO3aUZr\nZLwD57z3MIbOj7dl+psdWsAzI9liXQgSs/NDAa1RHrnQ/T8gtmMhvGymg11gyVTy\n1olQF0zYp/4euRRPrnJsqe+RiKI59NXKeVJvJwIDAQABAoH/Vw4YJdBoOgpBVnR7\nNVzR4rNjzJPvVtHgtxQ4pKi7gNH3pTfKjPoFu3sOs7bYpKHSes0wHvMgLH7xbOt+\n7mnAlNDqJiZbeGl0uP0CmYU7tHhGZia4iIbB5sMFnNfsX5cVFXQKAE2W6aMLopzP\noz5pZcVTZ5R9ipEFo27i8ocx6V/GH0s6S6YaNM74mt7xLfB1IpNXn8DbOKyeuFEB\nJzA5cK0VIHD5P3m1C0LssvAetD+3MSgpDdWg5bGQYJyvLRbwpY1RDXhG+MvhClQ3\nzb9SI3b9vgt5LoINwm5dndH14kc46KrGHq98uSX+hc4wZQXFVYKriw660UqFVkvo\nMUnxAoGBAM4jnz+cVgnNOklv2CkpeTmQBckEL1emqk7YCycyKJpZEfzizM6vGvVp\nQ6d206rmGwZKzRNwA6pflunbtRgEvpGg3+UpaAPws8ICc1+23lxXFIO9VMj4aA3J\nD15izEnoYQnREp73J6zMFrCFvlTyzV/kCMMlMOMuRBuZP96Ec8R7AoGBAMtuX2Zr\niG0tpT7jtwFaMwJAeQkA2K9W1ZndmL5bd02/JVttk94LlBuphw3SJQyzpm49KHH/\nojj8vDK77hx0JGtg0IRxk9MVuOqTaSDW1qs/XEwfRsKlu0ZWdlyv1RJhBpwLq22a\ncKiOPLgkv8BsksJxWYozn3tyKZ202Vinc05FAoGAWUjwlvq+AWzUzC4XcGy4Rwhs\nmQ6o+swDhDbrTcTZ9VCTC/eoQfVyNc2sMgKz15fnKKiJftvLmqXgldZF7+tOJ4GI\nuJdgqCQ/l8677j8kvNYy2oTNDmcZmq71aypb1b9DGJ9n81aV5rBXO4oSUHUCI4B/\nkxauwBewBEwyb8zq37UCgYEAuobCTIvOtKTEvmxXMZM98BOjpwCyrdrF0eYglnIR\nayUXoMRRb6hYLiD8J00HEaVrteq3ugc1/GCoQzCTvF01w4sqxjXJ4CMrwVJWc3jp\nrm1TOKz42aLSm0TOFGBY0xhJ0izVC5MvNy3sOL/hXhNvYYBDJHf/BFaCGAtpumMX\nU60CgYBeRROQTmgtNEZGPJGhOx3N+hv75buyUnuxfIeHkbRMNC3i6+VsDZ/NiS0I\n276s7yG09/wA4VVu/XPRkQSVDBQ4Yfe9U/uAXjJ6ERkhYDwAWHQNZSe4C0mAyqz2\nEcM1JYM9/0YxekDQC6lOQfgLG+TLuONqAeV9+GxPRGoGI3Bz9Q==\n-----END RSA PRIVATE KEY-----\n";

const EC_SEC1_KEY_PEM: &str = "-----BEGIN EC PRIVATE KEY-----\nMHcCAQEEIEq5USLmdFPuLr88OLcgsig1jFBR4EBG1G0F2NH2hxssoAoGCCqGSM49\nAwEHoUQDQgAEXnZm8Pe0O7yTeql6/Nm/+IisvgsMTwp5EIpc2JMG9zToFkWCES9C\nFk0JwwNlPDBJpHelWLcPsAYQVEHn0nhb2g==\n-----END EC PRIVATE KEY-----\n";

/// Writes `contents` to a per-process temp file and returns its path.
fn write_fixture(name: &str, contents: &str) -> String {
    let path = std::env::temp_dir().join(format!("rws_tls_tests_{}_{}", std::process::id(), name));
    std::fs::write(&path, contents).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn load_certs_reads_every_certificate_in_the_file() {
    let path = write_fixture("chain.pem", &format!("{CERT_PEM}{CERT_PEM}"));
    assert_eq!(2, load_certs(&path).unwrap().len());
}

#[test]
fn load_certs_skips_non_certificate_sections() {
    let path = write_fixture("cert_and_key.pem", &format!("{RSA_PKCS8_KEY_PEM}{CERT_PEM}"));
    assert_eq!(1, load_certs(&path).unwrap().len());
}

#[test]
fn load_certs_missing_file_is_an_error() {
    let err = load_certs("/nonexistent/rws/cert.pem").unwrap_err();
    assert!(err.contains("failed to read cert file"), "{err}");
}

#[test]
fn load_key_accepts_pkcs8_pkcs1_and_sec1() {
    for (name, pem) in [
        ("pkcs8.pem", RSA_PKCS8_KEY_PEM),
        ("pkcs1.pem", RSA_PKCS1_KEY_PEM),
        ("sec1.pem", EC_SEC1_KEY_PEM),
    ] {
        let path = write_fixture(name, pem);
        assert!(load_key(&path).is_ok(), "{name} should load");
    }
}

#[test]
fn load_key_with_no_key_in_file_says_so() {
    let path = write_fixture("only_cert.pem", CERT_PEM);
    let err = load_key(&path).unwrap_err();
    assert!(err.contains("no private key found"), "{err}");
}

#[test]
fn load_key_missing_file_is_an_error() {
    let err = load_key("/nonexistent/rws/key.pem").unwrap_err();
    assert!(err.contains("failed to read key file"), "{err}");
}

#[test]
fn load_certified_key_pairs_cert_with_key() {
    let cert = write_fixture("pair_cert.pem", CERT_PEM);
    let key = write_fixture("pair_key.pem", RSA_PKCS1_KEY_PEM);
    assert_eq!(1, load_certified_key(&cert, &key).unwrap().cert.len());
}
