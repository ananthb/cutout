use mail_parser::MessageParser;

/// Parsed representation of an inbound email.
pub struct ParsedEmail {
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
        subject: message.subject().unwrap_or("").to_string(),
        message_id: message.message_id().map(|s| s.to_string()),
        references: message.references().as_text().map(|s| s.to_string()),
        text_body: message.body_text(0).map(|s| s.to_string()),
        html_body: message.body_html(0).map(|s| s.to_string()),
    })
}

/// Parameters for building a raw RFC 2822 email.
struct EmailParams<'a> {
    from: &'a str,
    to: &'a str,
    reply_to: Option<&'a str>,
    subject: &'a str,
    extra_headers: &'a [(&'a str, &'a str)],
    text_body: Option<&'a str>,
    html_body: Option<&'a str>,
    proxy_domain: &'a str,
}

/// RFC 2047 encode a subject if it contains non-ASCII characters.
fn encode_subject(subject: &str) -> String {
    if subject.is_ascii() {
        subject.to_string()
    } else {
        use base64::Engine;
        format!(
            "=?UTF-8?B?{}?=",
            base64::engine::general_purpose::STANDARD.encode(subject.as_bytes())
        )
    }
}

/// Build a raw RFC 2822 email from parts.
fn build_email(params: &EmailParams<'_>) -> Vec<u8> {
    let EmailParams {
        from,
        to,
        reply_to,
        subject,
        extra_headers,
        text_body,
        html_body,
        proxy_domain,
    } = params;

    let mut out = String::new();
    let message_id = format!("<{}@{}>", uuid::Uuid::new_v4(), proxy_domain);

    out.push_str(&format!("Message-ID: {message_id}\r\n"));
    #[cfg(not(test))]
    out.push_str(&format!(
        "Date: {}\r\n",
        js_sys::Date::new_0()
            .to_utc_string()
            .as_string()
            .unwrap_or_default()
    ));
    out.push_str(&format!("From: {from}\r\n"));
    out.push_str(&format!("To: {to}\r\n"));
    out.push_str(&format!("Subject: {}\r\n", encode_subject(subject)));
    out.push_str("MIME-Version: 1.0\r\n");

    if let Some(rt) = reply_to {
        out.push_str(&format!("Reply-To: {rt}\r\n"));
    }

    for (name, value) in *extra_headers {
        // Guard against header injection
        if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            continue;
        }
        out.push_str(&format!("{name}: {value}\r\n"));
    }

    match (html_body, text_body) {
        (Some(html), Some(text)) => {
            let boundary = format!("----=_Part_{}", uuid::Uuid::new_v4().simple());
            out.push_str(&format!(
                "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n"
            ));
            out.push_str("\r\n");
            out.push_str(&format!("--{boundary}\r\n"));
            out.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            out.push_str("Content-Transfer-Encoding: 8bit\r\n");
            out.push_str("\r\n");
            out.push_str(text);
            out.push_str("\r\n");
            out.push_str(&format!("--{boundary}\r\n"));
            out.push_str("Content-Type: text/html; charset=utf-8\r\n");
            out.push_str("Content-Transfer-Encoding: 8bit\r\n");
            out.push_str("\r\n");
            out.push_str(html);
            out.push_str("\r\n");
            out.push_str(&format!("--{boundary}--\r\n"));
        }
        (Some(html), None) => {
            out.push_str("Content-Type: text/html; charset=utf-8\r\n");
            out.push_str("Content-Transfer-Encoding: 8bit\r\n");
            out.push_str("\r\n");
            out.push_str(html);
        }
        (_, Some(text)) => {
            out.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            out.push_str("Content-Transfer-Encoding: 8bit\r\n");
            out.push_str("\r\n");
            out.push_str(text);
        }
        (None, None) => {
            out.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            out.push_str("\r\n");
        }
    }

    out.into_bytes()
}

/// Build a forwarded email with rewritten headers.
pub fn build_forwarded_email(
    original: &ParsedEmail,
    from: &str,
    to: &str,
    reply_to: &str,
    original_from: &str,
    proxy_domain: &str,
) -> Vec<u8> {
    let mut headers: Vec<(&str, &str)> = vec![
        ("X-Original-From", original_from),
        ("X-Cutout-Forwarded", "1"),
    ];

    if let Some(ref msg_id) = original.message_id {
        headers.push(("In-Reply-To", msg_id));
    }
    if let Some(ref refs) = original.references {
        headers.push(("References", refs));
    }

    build_email(&EmailParams {
        from,
        to,
        reply_to: Some(reply_to),
        subject: &original.subject,
        extra_headers: &headers,
        text_body: original.text_body.as_deref(),
        html_body: original.html_body.as_deref(),
        proxy_domain,
    })
}

/// Build a reply-routed email (user reply going back to the original sender).
pub fn build_reply_email(
    original: &ParsedEmail,
    from_alias: &str,
    to_original_sender: &str,
    proxy_domain: &str,
) -> Vec<u8> {
    let mut headers: Vec<(&str, &str)> = vec![("X-Cutout-Forwarded", "1")];

    if let Some(ref msg_id) = original.message_id {
        headers.push(("In-Reply-To", msg_id));
    }
    if let Some(ref refs) = original.references {
        headers.push(("References", refs));
    }

    build_email(&EmailParams {
        from: from_alias,
        to: to_original_sender,
        reply_to: Some(from_alias),
        subject: &original.subject,
        extra_headers: &headers,
        text_body: original.text_body.as_deref(),
        html_body: original.html_body.as_deref(),
        proxy_domain,
    })
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
    }

    #[test]
    fn forward_rewrites_headers() {
        let parsed = parse_email(SIMPLE_EMAIL).expect("should parse");
        let forwarded = build_forwarded_email(
            &parsed,
            "reply+abc@proxy.example.com",
            "user@gmail.com",
            "reply+abc@proxy.example.com",
            "alice@example.org",
            "proxy.example.com",
        );
        let raw = String::from_utf8(forwarded).expect("valid utf8");

        assert!(raw.contains("From: reply+abc@proxy.example.com"));
        assert!(raw.contains("To: user@gmail.com"));
        assert!(raw.contains("Reply-To: reply+abc@proxy.example.com"));
        assert!(raw.contains("X-Original-From: alice@example.org"));
        assert!(raw.contains("X-Cutout-Forwarded: 1"));
        assert!(raw.contains("Subject: Hello from Alice"));
        assert!(raw.contains("test email"));
    }

    #[test]
    fn reply_rewrites_from_to_alias() {
        let reply_raw = b"From: user@gmail.com\r\n\
            To: reply+abc@proxy.example.com\r\n\
            Subject: Re: Hello from Alice\r\n\
            Message-ID: <reply999@gmail.com>\r\n\
            Content-Type: text/plain; charset=utf-8\r\n\
            \r\n\
            Thanks Alice.\r\n";

        let parsed = parse_email(reply_raw).expect("should parse");
        let reply = build_reply_email(
            &parsed,
            "shop123@proxy.example.com",
            "alice@example.org",
            "proxy.example.com",
        );
        let raw = String::from_utf8(reply).expect("valid utf8");

        assert!(raw.contains("From: shop123@proxy.example.com"));
        assert!(raw.contains("To: alice@example.org"));
        assert!(raw.contains("Reply-To: shop123@proxy.example.com"));
        assert!(raw.contains("X-Cutout-Forwarded: 1"));
    }

    #[test]
    fn forwarded_email_is_reparseable() {
        let parsed = parse_email(SIMPLE_EMAIL).expect("should parse");
        let forwarded = build_forwarded_email(
            &parsed,
            "reply+test@proxy.example.com",
            "user@gmail.com",
            "reply+test@proxy.example.com",
            "alice@example.org",
            "proxy.example.com",
        );
        let reparsed = parse_email(&forwarded).expect("should reparse");
        assert_eq!(reparsed.subject, "Hello from Alice");
    }
}
