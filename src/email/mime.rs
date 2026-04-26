use base64::Engine;
use mail_parser::{MessageParser, MimeHeaders};

/// Parsed representation of an inbound email: only the fields we need to
/// reconstruct an outbound message via Cloudflare Email Service.
pub struct ParsedEmail {
    pub from_name: Option<String>,
    pub from_address: Option<String>,
    pub subject: String,
    pub message_id: Option<String>,
    pub references: Option<String>,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
}

/// Parse raw MIME bytes into a structured email.
pub fn parse_email(raw: &[u8]) -> Option<ParsedEmail> {
    let message = MessageParser::default().parse(raw)?;

    Some(ParsedEmail {
        from_name: message
            .from()
            .and_then(|h| h.as_list())
            .and_then(|list| list.first())
            .and_then(|a| a.name.as_ref())
            .map(|s| s.to_string()),
        from_address: message
            .from()
            .and_then(|h| h.as_list())
            .and_then(|list| list.first())
            .and_then(|a| a.address.as_ref())
            .map(|s| s.to_string()),
        subject: message.subject().unwrap_or("").to_string(),
        message_id: message.message_id().map(|s| s.to_string()),
        references: message.references().as_text().map(|s| s.to_string()),
        text_body: message.body_text(0).map(|s| s.to_string()),
        html_body: message.body_html(0).map(|s| s.to_string()),
    })
}

/// Parse raw MIME bytes and return the HTML body with `cid:` image
/// references rewritten to inline `data:` URIs. Returns `None` when the
/// message has no `text/html` part (text-only emails are handled by the
/// caller using `text_body`).
pub fn inlined_html(raw: &[u8]) -> Option<String> {
    let message = MessageParser::default().parse(raw)?;
    let html_part = message.html_part(0)?;
    if !html_part.is_text_html() {
        return None;
    }
    let mut html = match std::str::from_utf8(html_part.contents()) {
        Ok(s) => s.to_string(),
        Err(_) => return None,
    };

    for part in &message.parts {
        let cid = match part.content_id() {
            Some(c) => c.trim_matches(['<', '>', ' ']).to_string(),
            None => continue,
        };
        if cid.is_empty() {
            continue;
        }
        let bytes = part.contents();
        if bytes.is_empty() {
            continue;
        }
        let mime_type = part
            .content_type()
            .map(|ct| match ct.subtype() {
                Some(sub) => format!("{}/{}", ct.ctype(), sub),
                None => ct.ctype().to_string(),
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let data_uri = format!("data:{mime_type};base64,{b64}");

        for prefix in ["cid:", "CID:", "Cid:"] {
            let needle = format!("{prefix}{cid}");
            if html.contains(&needle) {
                html = html.replace(&needle, &data_uri);
            }
        }
    }

    Some(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_EMAIL: &[u8] = b"From: Alice <alice@example.org>\r\n\
        To: shop123@proxy.example.com\r\n\
        Subject: Hello from Alice\r\n\
        Message-ID: <original123@example.org>\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Hi there, this is a test email.\r\n";

    const MULTIPART_EMAIL: &[u8] = b"From: Bob <bob@shop.com>\r\n\
        To: orders@proxy.example.com\r\n\
        Subject: Your order confirmation\r\n\
        Message-ID: <order456@shop.com>\r\n\
        In-Reply-To: <prev789@proxy.example.com>\r\n\
        References: <prev789@proxy.example.com>\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: multipart/alternative; boundary=\"boundary42\"\r\n\
        \r\n\
        --boundary42\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\
        \r\n\
        Your order #1234 is confirmed.\r\n\
        --boundary42\r\n\
        Content-Type: text/html; charset=utf-8\r\n\
        \r\n\
        <h1>Your order #1234 is confirmed.</h1>\r\n\
        --boundary42--\r\n";

    #[test]
    fn parse_simple_email() {
        let parsed = parse_email(SIMPLE_EMAIL).expect("should parse");
        assert_eq!(parsed.subject, "Hello from Alice");
        assert_eq!(
            parsed.message_id.as_deref(),
            Some("original123@example.org")
        );
        assert!(parsed.text_body.as_ref().unwrap().contains("test email"));
    }

    #[test]
    fn parse_multipart_email() {
        let parsed = parse_email(MULTIPART_EMAIL).expect("should parse");
        assert_eq!(parsed.subject, "Your order confirmation");
        assert!(parsed.text_body.as_ref().unwrap().contains("order #1234"));
        assert!(parsed.html_body.as_ref().unwrap().contains("<h1>"));
        assert_eq!(
            parsed.references.as_deref(),
            Some("prev789@proxy.example.com")
        );
    }

    const HTML_WITH_CID: &[u8] = b"From: Bob <bob@shop.com>\r\n\
        To: orders@proxy.example.com\r\n\
        Subject: Receipt\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: multipart/related; boundary=\"REL\"\r\n\
        \r\n\
        --REL\r\n\
        Content-Type: text/html; charset=utf-8\r\n\
        \r\n\
        <p>See <img src=\"cid:logo123\"></p>\r\n\
        --REL\r\n\
        Content-Type: image/png\r\n\
        Content-ID: <logo123>\r\n\
        Content-Transfer-Encoding: base64\r\n\
        \r\n\
        iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQMAAAAl21bKAAAAA1BMVEX///+nxBvIAAAAC0lEQVQI12NgAAIAAAUAAeImBZsAAAAASUVORK5CYII=\r\n\
        --REL--\r\n";

    #[test]
    fn inlines_cid_image_to_data_uri() {
        let html = inlined_html(HTML_WITH_CID).expect("html present");
        assert!(
            html.contains("data:image/png;base64,"),
            "expected data URI, got: {html}"
        );
        assert!(!html.contains("cid:logo123"), "cid: still present: {html}");
    }

    #[test]
    fn inlined_html_returns_none_for_text_only() {
        // SIMPLE_EMAIL has no HTML body.
        assert!(inlined_html(SIMPLE_EMAIL).is_none());
    }

    #[test]
    fn inlined_html_passthrough_when_no_cid() {
        let html = inlined_html(MULTIPART_EMAIL).expect("html present");
        assert!(html.contains("<h1>"), "html body lost: {html}");
    }

    #[test]
    fn test_inline_pgp_preservation() {
        let pgp_email = b"From: Alice <alice@example.org>\r\n\
            To: shop@proxy.com\r\n\
            Subject: Secret\r\n\
            \r\n\
            -----BEGIN PGP SIGNED MESSAGE-----\r\n\
            Hash: SHA256\r\n\
            \r\n\
            This is a secret message.\r\n\
            -----BEGIN PGP SIGNATURE-----\r\n\
            Version: GnuPG v2\r\n\
            \r\n\
            iQEcBAEBCAAGBQJ...\r\n\
            -----END PGP SIGNATURE-----\r\n";

        let parsed = parse_email(pgp_email).expect("should parse");
        let body = parsed.text_body.unwrap();

        assert!(body.contains("-----BEGIN PGP SIGNED MESSAGE-----"));
        assert!(body.contains("This is a secret message."));
        assert!(body.contains("-----END PGP SIGNATURE-----"));
    }
}
