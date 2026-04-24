use mail_parser::MessageParser;

/// Parsed representation of an inbound email — only the fields we need to
/// reconstruct an outbound message via Cloudflare Email Service.
pub struct ParsedEmail {
    pub from_header: Option<String>,
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
        from_header: message
            .header("From")
            .and_then(|h| h.as_text())
            .map(|s| s.to_string()),
        subject: message.subject().unwrap_or("").to_string(),
        message_id: message.message_id().map(|s| s.to_string()),
        references: message.references().as_text().map(|s| s.to_string()),
        text_body: message.body_text(0).map(|s| s.to_string()),
        html_body: message.body_html(0).map(|s| s.to_string()),
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
        assert_eq!(
            parsed.references.as_deref(),
            Some("prev789@proxy.example.com")
        );
    }
}
