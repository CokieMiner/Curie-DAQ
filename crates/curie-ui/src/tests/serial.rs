//! Serial helpers.

use crate::serial::parse_escaped_string;

#[test]
fn test_parse_escaped_string() {
    assert_eq!(parse_escaped_string(r"X 20\r"), b"X 20\r");
    assert_eq!(parse_escaped_string(r"A\r\n"), b"A\r\n");
    assert_eq!(parse_escaped_string("HELLO\tWORLD"), b"HELLO\tWORLD");
    assert_eq!(parse_escaped_string(""), b"");
}
