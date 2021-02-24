use heng_utils::crypto::{hex_hmac_sha256, hex_sha256};
use smallvec::SmallVec;

fn uri_encode(input: &[u8]) -> String {
    const HEX_UPPERCASE_TABLE: &[u8] = b"0123456789ABCDEF";

    let mut s = String::new();

    unsafe {
        let buf = s.as_mut_vec();
        buf.reserve(input.len());

        for &byte in input {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'~' | b'.' => {
                    buf.push(byte)
                }
                _ => {
                    buf.push(b'%');
                    buf.push(HEX_UPPERCASE_TABLE[usize::from(byte.wrapping_shr(4))]);
                    buf.push(HEX_UPPERCASE_TABLE[usize::from(byte & 15)]);
                }
            }
        }
    }

    s
}

fn push_nvs(s: &mut String, nvs: &[(impl AsRef<str>, impl AsRef<str>)]) {
    let total: usize = nvs
        .iter()
        .map(|(n, v)| n.as_ref().len() + v.as_ref().len())
        .sum();
    s.reserve(total);
    if let Some((first, remain)) = nvs.split_first() {
        {
            let &(ref name, ref value) = first;
            s.push_str(name.as_ref());
            s.push('=');
            s.push_str(value.as_ref());
        }
        for &(ref name, ref value) in remain {
            s.push('&');
            s.push_str(name.as_ref());
            s.push('=');
            s.push_str(value.as_ref());
        }
    }
}

static SIGNED_HEADERS: &[&str] = &[
    "content-type",
    "x-heng-accesskey",
    "x-heng-nonce",
    "x-heng-timestamp",
];

const EMPTY_SHA256_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub fn calc_signature(
    method: &http::Method,
    path: &str,
    query: &str,
    headers: &http::HeaderMap,
    body: &[u8],
    secret_key: &str,
) -> String {
    let mut request_string = String::new();

    {
        request_string += method.as_str();
        request_string.push('\n');
    }
    {
        request_string += path;
        request_string.push('\n');
    }

    {
        if !query.is_empty() {
            let mut nvs: SmallVec<[(String, String); 16]> = SmallVec::new();
            for (name, value) in form_urlencoded::parse(query.as_bytes()) {
                let name = uri_encode(name.as_bytes());
                let value = uri_encode(value.as_bytes());
                nvs.push((name, value));
            }
            nvs.sort();
            push_nvs(&mut request_string, &nvs);
        }
        request_string.push('\n');
    }

    {
        let mut nvs: SmallVec<[(&str, String); 16]> = SmallVec::new();
        for &name in SIGNED_HEADERS {
            if let Some(value) = headers.get(name) {
                let value = uri_encode(value.as_bytes());
                nvs.push((name, value));
            }
        }
        if !nvs.is_empty() {
            nvs.sort();
            push_nvs(&mut request_string, &nvs);
        }
        request_string.push('\n');
    }
    {
        if body.is_empty() {
            request_string += EMPTY_SHA256_HASH;
        } else {
            request_string += &hex_sha256(body);
        }
        request_string.push('\n');
    }
    hex_hmac_sha256(secret_key.as_bytes(), request_string.as_bytes())
}

#[cfg(test)]
mod tests {
    use http::header::{HeaderName, HeaderValue};

    use super::calc_signature;

    macro_rules! hname {
        ($str:literal) => {
            HeaderName::from_static($str)
        };
    }

    macro_rules! hvalue {
        ($str:literal) => {
            HeaderValue::from_static($str)
        };
    }

    #[test]
    fn check() {
        let mut map = http::HeaderMap::new();
        map.insert(hname!("x-heng-accesskey"), hvalue!("example-ak"));
        map.insert(hname!("x-heng-nonce"), hvalue!("random"));
        map.insert(hname!("x-heng-timestamp"), hvalue!("1614130246801"));

        let signature = calc_signature(
            &http::Method::GET,
            "/v1/judgers/token",
            "",
            &map,
            &[],
            "example-sk",
        );

        assert_eq!(
            signature,
            "5a9b2583678fd88de7ebb5a422ba3d5f6475ab729b892aa05b94c302b79bee1e"
        )
    }
}
