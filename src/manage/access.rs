//! Cloudflare Access JWT verification.

use wasm_bindgen::JsCast;
use worker::*;

/// Verify the Cloudflare Access JWT and return the authenticated email.
pub async fn verify_access(req: &Request, env: &Env) -> Option<String> {
    // Access sets the JWT as both a header and a cookie — check both
    let token = match req.headers().get("Cf-Access-Jwt-Assertion").ok()? {
        Some(t) => t,
        None => match get_cookie(req, "CF_Authorization") {
            Some(t) => t,
            None => {
                console_log!("No Cf-Access-Jwt-Assertion header or CF_Authorization cookie");
                return None;
            }
        },
    };

    let aud = match env.var("CF_ACCESS_AUD").map(|v| v.to_string()).ok() {
        Some(a) if !a.is_empty() => a,
        _ => {
            console_log!("CF_ACCESS_AUD not set or empty");
            return None;
        }
    };

    let team_domain = match env.var("CF_ACCESS_TEAM").map(|v| v.to_string()).ok() {
        Some(t) if !t.is_empty() => t,
        _ => {
            console_log!("CF_ACCESS_TEAM not set or empty");
            return None;
        }
    };

    console_log!(
        "Verifying JWT: aud={aud}, team={team_domain}, token_len={}",
        token.len()
    );

    match verify_cf_jwt(&token, &aud, &team_domain).await {
        Ok(email) => {
            console_log!("Access verified: {email}");
            Some(email)
        }
        Err(e) => {
            console_log!("Access JWT verification failed: {e}");
            None
        }
    }
}

/// Verify a Cloudflare Access RS256 JWT. Returns the email claim on success.
async fn verify_cf_jwt(token: &str, expected_aud: &str, team_domain: &str) -> Result<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(Error::from("Invalid JWT format"));
    }

    let header: serde_json::Value = decode_jwt_part(parts[0])?;
    let payload: serde_json::Value = decode_jwt_part(parts[1])?;

    // Check algorithm
    if header.get("alg").and_then(|v| v.as_str()) != Some("RS256") {
        return Err(Error::from("Unsupported JWT algorithm"));
    }

    let kid = header
        .get("kid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::from("Missing kid in JWT header"))?;

    // Verify audience
    let aud_valid = match payload.get("aud") {
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(expected_aud)),
        Some(serde_json::Value::String(s)) => s == expected_aud,
        _ => false,
    };
    if !aud_valid {
        return Err(Error::from("JWT audience mismatch"));
    }

    // Check expiry
    let now = (js_sys::Date::now() / 1000.0) as u64;
    let exp = payload
        .get("exp")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::from("Missing exp claim"))?;
    if now > exp {
        return Err(Error::from("JWT expired"));
    }

    // Fetch JWKS and find the matching key
    let certs_url = format!("https://{team_domain}.cloudflareaccess.com/cdn-cgi/access/certs");
    let mut resp = Fetch::Url(Url::parse(&certs_url).map_err(|_| Error::from("Bad certs URL"))?)
        .send()
        .await?;
    let jwks: serde_json::Value = resp.json().await?;

    let keys = jwks
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::from("Invalid JWKS response"))?;

    let jwk = keys
        .iter()
        .find(|k| k.get("kid").and_then(|v| v.as_str()) == Some(kid))
        .ok_or_else(|| Error::from("No matching key in JWKS"))?;

    // Import the RSA public key and verify signature
    let crypto = get_subtle()?;

    let algorithm = js_sys::Object::new();
    js_sys::Reflect::set(&algorithm, &"name".into(), &"RSASSA-PKCS1-v1_5".into())
        .map_err(|_| Error::from("reflect error"))?;
    js_sys::Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())
        .map_err(|_| Error::from("reflect error"))?;

    let jwk_js = serde_json::to_string(jwk).map_err(|e| Error::from(format!("JSON: {e}")))?;
    let jwk_obj: js_sys::Object = js_sys::JSON::parse(&jwk_js)
        .map_err(|_| Error::from("JWK parse error"))?
        .dyn_into()
        .map_err(|_| Error::from("JWK not an object"))?;

    let usages = js_sys::Array::new();
    usages.push(&"verify".into());

    let key_promise = crypto
        .import_key_with_object("jwk", &jwk_obj, &algorithm, false, &usages)
        .map_err(|_| Error::from("importKey failed"))?;
    let crypto_key: web_sys::CryptoKey = wasm_bindgen_futures::JsFuture::from(key_promise)
        .await
        .map_err(|_| Error::from("importKey await failed"))?
        .into();

    let signed_input = format!("{}.{}", parts[0], parts[1]);
    let signature = base64_url_decode(parts[2])?;

    let verify_promise = crypto
        .verify_with_object_and_buffer_source_and_buffer_source(
            &algorithm,
            &crypto_key,
            &js_sys::Uint8Array::from(signature.as_slice()),
            &js_sys::Uint8Array::from(signed_input.as_bytes()),
        )
        .map_err(|_| Error::from("verify call failed"))?;

    let valid = wasm_bindgen_futures::JsFuture::from(verify_promise)
        .await
        .map_err(|_| Error::from("verify await failed"))?
        .as_bool()
        .unwrap_or(false);

    if !valid {
        return Err(Error::from("JWT signature invalid"));
    }

    payload
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::from("Missing email in JWT"))
}

fn decode_jwt_part(part: &str) -> Result<serde_json::Value> {
    let bytes = base64_url_decode(part)?;
    serde_json::from_slice(&bytes).map_err(|e| Error::from(format!("JWT decode: {e}")))
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| Error::from(format!("base64: {e}")))
}

fn get_subtle() -> Result<web_sys::SubtleCrypto> {
    let global = js_sys::global();
    let crypto = js_sys::Reflect::get(&global, &"crypto".into())
        .map_err(|_| Error::from("Failed to get crypto"))?;
    let crypto: web_sys::Crypto = crypto
        .dyn_into()
        .map_err(|_| Error::from("Not a Crypto object"))?;
    Ok(crypto.subtle())
}

/// Extract a cookie value by name from the Cookie header.
fn get_cookie(req: &Request, name: &str) -> Option<String> {
    let header = req.headers().get("Cookie").ok()??;
    let prefix = format!("{name}=");
    header
        .split(';')
        .map(|s| s.trim())
        .find(|s| s.starts_with(&prefix))
        .map(|s| s[prefix.len()..].to_string())
}
