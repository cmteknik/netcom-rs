use std::fmt;

#[derive(Debug, PartialEq)]
pub enum NetstringError {
    Incomplete,
    Malformed,
}

impl fmt::Display for NetstringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetstringError::Incomplete => write!(f, "Incomplete netstring"),
            NetstringError::Malformed => write!(f, "Malformed netstring"),
        }
    }
}

impl std::error::Error for NetstringError {}

pub fn parse_netstring(buf: &[u8]) -> Result<&[u8], NetstringError> {
    if buf.len() < 3 {
        return Err(NetstringError::Incomplete);
    }

    let mut i = 0;
    let mut string_length: usize = 0;

    while i < buf.len() {
        let c = buf[i];
        i += 1;
        match c {
            b'0'..=b'9' => string_length = string_length * 10 + (c - b'0') as usize,
            b':' => break,
            _ => return Err(NetstringError::Malformed),
        }
    }

    if i == buf.len() {
        return Err(NetstringError::Incomplete);
    }

    if buf.len() < i + string_length + 1 {
        return Err(NetstringError::Incomplete);
    }

    if buf[i + string_length] != b',' {
        return Err(NetstringError::Malformed);
    }

    return Ok(&buf[i..(i + string_length)]);
}

pub trait ToNetstring {
    fn to_netstring(&self) -> Vec<u8>;
}

impl ToNetstring for String {
    fn to_netstring(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(self.len().to_string().as_bytes());
        buffer.push(b':');
        buffer.extend(self.as_bytes());
        buffer.push(b',');
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_incomplete_for_short_inputs() {
        let cases = ["", "0", "0:"];
        for case in cases {
            assert_eq!(
                parse_netstring(case.as_bytes()),
                Err(NetstringError::Incomplete)
            );
        }
    }

    #[test]
    fn should_fail_if_non_digits_before_colon() {
        assert_eq!(
            parse_netstring("11:".as_bytes()),
            Err(NetstringError::Incomplete)
        );
        assert_eq!(
            parse_netstring("1X:".as_bytes()),
            Err(NetstringError::Malformed)
        );
    }

    #[test]
    fn should_require_one_or_more_digits_before_colon() {
        assert_eq!(
            parse_netstring(":abc,".as_bytes()),
            Err(NetstringError::Malformed)
        )
    }

    #[test]
    fn should_return_incomplete_if_not_enough_data() {
        let cases = ["5:abc", "5:abcd", "5:abcde"];
        for case in cases {
            assert_eq!(
                parse_netstring(case.as_bytes()),
                Err(NetstringError::Incomplete)
            );
        }
    }

    #[test]
    fn should_fail_if_no_comma_at_end_of_string() {
        assert_eq!(
            parse_netstring("5:abcde!".as_bytes()),
            Err(NetstringError::Malformed)
        );
    }

    #[test]
    fn should_return_payload_and_offset_for_valid_netstrings() {
        let cases = [
            ("0:,", ""),
            ("1:x,", "x"),
            ("5:abcde,", "abcde"),
            ("16:abcdefghijklmnop,", "abcdefghijklmnop"),
        ];
        for case in cases {
            assert_eq!(parse_netstring(case.0.as_bytes()), Ok(case.1.as_bytes()));
        }
    }
}
