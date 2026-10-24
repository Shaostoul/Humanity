//! An S3-compatible bucket as a backup destination (2026-10-22): four requests (PUT, GET,
//! DELETE, ListObjectsV2) signed with AWS Signature Version 4, which every S3-compatible
//! server speaks (AWS, Cloudflare R2, Backblaze B2, Wasabi, and self-hosted Garage or MinIO).
//! Written here rather than pulling in an SDK: the protocol is small and stable, and the
//! signing is checked against AWS's own published test vectors below.
//!
//! The secret access key never leaves this module except as an HMAC key: it is not in any URL
//! (requests are signed in the `Authorization` header, never presigned), not in any error
//! (errors carry S3's error code and the HTTP status only), and not in any log.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use super::dest::DestinationConfig;
use super::names::Listed;

type HmacSha256 = Hmac<Sha256>;

pub struct S3Dest {
    endpoint: url::Url,
    region: String,
    bucket: String,
    prefix: String,
    access_key_id: String,
    secret: String,
    path_style: bool,
    client: reqwest::Client,
}

impl S3Dest {
    pub fn new(cfg: &DestinationConfig) -> Result<Self, String> {
        let DestinationConfig::S3 { endpoint, region, bucket, prefix, access_key_id, secret_access_key, path_style } = cfg else {
            return Err("not an S3 destination".into());
        };
        let endpoint = url::Url::parse(endpoint.trim()).map_err(|_| "the endpoint is not an address".to_string())?;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("could not start the web client: {e}"))?;
        Ok(Self {
            endpoint,
            region: region.trim().to_string(),
            bucket: bucket.trim().to_string(),
            prefix: prefix.trim().trim_matches('/').to_string(),
            access_key_id: access_key_id.trim().to_string(),
            secret: secret_access_key.clone(),
            path_style: *path_style,
            client,
        })
    }

    fn key(&self, name: &str) -> String {
        if self.prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}/{name}", self.prefix)
        }
    }

    /// (url, host header, canonical path) for an object key, or for the bucket itself when `key` is empty.
    fn target(&self, key: &str) -> Result<(url::Url, String, String), String> {
        let host = self.endpoint.host_str().ok_or("no host")?.to_string();
        let port = self.endpoint.port().map(|p| format!(":{p}")).unwrap_or_default();
        let base_path = self.endpoint.path().trim_end_matches('/');
        let (host_header, path) = if self.path_style {
            let p = if key.is_empty() { format!("{base_path}/{}", self.bucket) } else { format!("{base_path}/{}/{key}", self.bucket) };
            (format!("{host}{port}"), p)
        } else {
            (format!("{}.{host}{port}", self.bucket), format!("{base_path}/{key}"))
        };
        let canonical = uri_encode_path(&path);
        let url = url::Url::parse(&format!("{}://{host_header}{canonical}", self.endpoint.scheme())).map_err(|e| format!("bad object address: {e}"))?;
        Ok((url, host_header, canonical))
    }

    async fn send(&self, method: reqwest::Method, key: &str, query: &[(&str, String)], body: Vec<u8>) -> Result<reqwest::Response, String> {
        let (mut url, host, path) = self.target(key)?;
        let payload_hash = hex::encode(Sha256::digest(&body));
        let amz_date = amz_now();
        let headers = vec![
            ("host".to_string(), host),
            ("x-amz-content-sha256".to_string(), payload_hash.clone()),
            ("x-amz-date".to_string(), amz_date.clone()),
        ];
        let q: Vec<(String, String)> = query.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let auth = authorization(&self.access_key_id, &self.secret, &self.region, "s3", method.as_str(), &path, &q, &headers, &payload_hash, &amz_date);
        if !q.is_empty() {
            url.set_query(Some(&canonical_query(&q)));
        }
        let req = self
            .client
            .request(method, url)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", amz_date)
            .header("authorization", auth)
            .body(body);
        req.send().await.map_err(|e| format!("could not reach the bucket's server ({})", plain_error(&e)))
    }

    pub async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let r = self.send(reqwest::Method::PUT, &self.key(name), &[], bytes).await?;
        ok_or_s3_error(r).await.map(|_| ())
    }

    pub async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let r = self.send(reqwest::Method::GET, &self.key(name), &[], Vec::new()).await?;
        if r.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let r = ok_or_s3_error(r).await?;
        r.bytes().await.map(|b| Some(b.to_vec())).map_err(|e| format!("reading from the bucket ({})", plain_error(&e)))
    }

    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let r = self.send(reqwest::Method::DELETE, &self.key(name), &[], Vec::new()).await?;
        if r.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        ok_or_s3_error(r).await.map(|_| ())
    }

    pub async fn list(&self) -> Result<Vec<Listed>, String> {
        let mut out = Vec::new();
        let prefix = if self.prefix.is_empty() { String::new() } else { format!("{}/", self.prefix) };
        let mut token: Option<String> = None;
        loop {
            let mut q = vec![("list-type", "2".to_string()), ("prefix", prefix.clone())];
            if let Some(t) = &token {
                q.push(("continuation-token", t.clone()));
            }
            let r = self.send(reqwest::Method::GET, "", &q, Vec::new()).await?;
            let r = ok_or_s3_error(r).await?;
            let xml = r.text().await.map_err(|e| format!("reading the bucket's list ({})", plain_error(&e)))?;
            let (items, next) = parse_list(&xml);
            for (key, size) in items {
                if let Some(name) = key.strip_prefix(&prefix) {
                    out.push(Listed { name: name.to_string(), size });
                }
            }
            match next {
                Some(t) => token = Some(t),
                None => break,
            }
        }
        Ok(out)
    }
}

/// A reqwest error in words that carry no address (its Display includes the URL, which is not
/// secret here, but the error kind is what a person needs).
fn plain_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "it took too long to answer"
    } else if e.is_connect() {
        "could not connect"
    } else if e.is_body() || e.is_decode() {
        "the answer was cut off"
    } else {
        "the request failed"
    }
}

async fn ok_or_s3_error(r: reqwest::Response) -> Result<reqwest::Response, String> {
    if r.status().is_success() {
        return Ok(r);
    }
    let status = r.status().as_u16();
    let body = r.text().await.unwrap_or_default();
    let code = between(&body, "<Code>", "</Code>").unwrap_or_default();
    let words = match code.as_str() {
        "AccessDenied" => "the access key is not allowed to do this",
        "InvalidAccessKeyId" => "the bucket's server does not know this access key ID",
        "SignatureDoesNotMatch" => "the secret access key does not match the access key ID",
        "NoSuchBucket" => "there is no bucket of that name",
        "RequestTimeTooSkewed" => "this computer's clock is too far from the bucket server's",
        "AuthorizationHeaderMalformed" => "the region is wrong for this bucket",
        "PermanentRedirect" => "the bucket is in another region or needs the other address style",
        _ => "",
    };
    if words.is_empty() {
        Err(format!("the bucket's server refused (HTTP {status}{})", if code.is_empty() { String::new() } else { format!(", {code}") }))
    } else {
        Err(format!("the bucket's server refused: {words} (HTTP {status}, {code})"))
    }
}

fn between(s: &str, open: &str, close: &str) -> Option<String> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(xml_unescape(&s[a..b]))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The keys and sizes in a ListObjectsV2 answer, and the token for the next page if there is one.
pub fn parse_list(xml: &str) -> (Vec<(String, u64)>, Option<String>) {
    let mut items = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Contents>") {
        let after = &rest[start + 10..];
        let Some(end) = after.find("</Contents>") else { break };
        let block = &after[..end];
        if let Some(key) = between(block, "<Key>", "</Key>") {
            let size = between(block, "<Size>", "</Size>").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            items.push((key, size));
        }
        rest = &after[end..];
    }
    let truncated = between(xml, "<IsTruncated>", "</IsTruncated>").is_some_and(|t| t.trim() == "true");
    let next = if truncated { between(xml, "<NextContinuationToken>", "</NextContinuationToken>") } else { None };
    (items, next)
}

/// RFC 3986 encoding as SigV4 wants it: unreserved characters stay, everything else is %XX.
fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn uri_encode_path(p: &str) -> String {
    let p = if p.is_empty() { "/" } else { p };
    uri_encode(p, true)
}

fn canonical_query(q: &[(String, String)]) -> String {
    let mut pairs: Vec<(String, String)> = q.iter().map(|(k, v)| (uri_encode(k, false), uri_encode(v, false))).collect();
    pairs.sort();
    pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

/// The `Authorization` header for one request (AWS Signature Version 4). `headers` are the
/// signed headers, lower-case names; `amz_date` is `YYYYMMDDTHHMMSSZ`.
#[allow(clippy::too_many_arguments)]
pub fn authorization(
    access_key_id: &str,
    secret: &str,
    region: &str,
    service: &str,
    method: &str,
    canonical_path: &str,
    query: &[(String, String)],
    headers: &[(String, String)],
    payload_hash: &str,
    amz_date: &str,
) -> String {
    let mut hs: Vec<(String, String)> = headers.iter().map(|(k, v)| (k.to_lowercase(), v.trim().to_string())).collect();
    hs.sort();
    let canonical_headers: String = hs.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_headers = hs.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(";");
    let canonical_request = format!(
        "{method}\n{canonical_path}\n{}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        canonical_query(query)
    );
    let date = &amz_date[..8];
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let string_to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", hex::encode(Sha256::digest(canonical_request.as_bytes())));
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac(&k_date, region);
    let k_service = hmac(&k_region, service);
    let k_signing = hmac(&k_service, "aws4_request");
    let signature = hex::encode(hmac(&k_signing, &string_to_sign));
    format!("AWS4-HMAC-SHA256 Credential={access_key_id}/{scope}, SignedHeaders={signed_headers}, Signature={signature}")
}

/// Now as `YYYYMMDDTHHMMSSZ` (UTC).
fn amz_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, mo, d) = super::civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!("{y:04}{mo:02}{d:02}T{:02}{:02}{:02}Z", t / 3600, (t % 3600) / 60, t % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    /// The signing matches AWS's published SigV4 test suite (awslabs/aws-c-auth,
    /// tests/aws-signing-test-suite/v4, read 2026-10-22): `get-vanilla` and
    /// `get-vanilla-query-order-key-case` (secret wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY,
    /// region us-east-1, service "service", 20150830T123600Z), and the S3 API reference's GET
    /// Object example with a Range header (the documentation key AKIAIOSFODNN7EXAMPLE).
    ///
    /// Seen red 2026-10-22 with the canonical query left unsorted: "query order: ...
    /// Signature=852baf9d507f14d925498277e9c258f123fac95b829db7a9ab4d87752289b0e4".
    #[test]
    fn signing_matches_the_published_test_vectors() {
        let h = |date: &str| vec![("host".to_string(), "example.amazonaws.com".to_string()), ("x-amz-date".to_string(), date.to_string())];
        let secret = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
        let a = authorization("AKIDEXAMPLE", secret, "us-east-1", "service", "GET", "/", &[], &h("20150830T123600Z"), EMPTY, "20150830T123600Z");
        assert!(a.ends_with("Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"), "get-vanilla: {a}");
        assert!(a.starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, "));
        let q = vec![("Param2".to_string(), "value2".to_string()), ("Param1".to_string(), "value1".to_string())];
        let a = authorization("AKIDEXAMPLE", secret, "us-east-1", "service", "GET", "/", &q, &h("20150830T123600Z"), EMPTY, "20150830T123600Z");
        assert!(a.ends_with("Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500"), "query order: {a}");

        let s3h = vec![
            ("Host".to_string(), "examplebucket.s3.amazonaws.com".to_string()),
            ("Range".to_string(), "bytes=0-9".to_string()),
            ("x-amz-content-sha256".to_string(), EMPTY.to_string()),
            ("x-amz-date".to_string(), "20130524T000000Z".to_string()),
        ];
        let a = authorization(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "s3",
            "GET",
            "/test.txt",
            &[],
            &s3h,
            EMPTY,
            "20130524T000000Z",
        );
        assert!(a.ends_with("Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"), "S3 GET Object: {a}");
    }

    #[test]
    fn the_list_answer_is_read_page_by_page() {
        let xml = "<?xml version=\"1.0\"?><ListBucketResult><IsTruncated>true</IsTruncated>\
            <Contents><Key>relay/owner</Key><Size>60</Size></Contents>\
            <Contents><Key>relay/a&amp;b</Key><Size>7</Size></Contents>\
            <NextContinuationToken>tok/1=</NextContinuationToken></ListBucketResult>";
        let (items, next) = parse_list(xml);
        assert_eq!(items, vec![("relay/owner".to_string(), 60), ("relay/a&b".to_string(), 7)]);
        assert_eq!(next.as_deref(), Some("tok/1="));
        let (_, last) = parse_list("<ListBucketResult><IsTruncated>false</IsTruncated></ListBucketResult>");
        assert_eq!(last, None);
        assert_eq!(canonical_query(&[("prefix".into(), "a/b c".into()), ("list-type".into(), "2".into())]), "list-type=2&prefix=a%2Fb%20c");
    }
}
