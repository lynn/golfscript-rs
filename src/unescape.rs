pub fn unescape(lexeme: &[u8], single_quoted: bool) -> Vec<u8> {
    let mut bytes = vec![];
    let mut escaping = false;
    for &b in lexeme.iter().take(lexeme.len() - 1).skip(1) {
        if escaping {
            if single_quoted {
                if b != b'\\' && b != b'\'' {
                    bytes.push(b'\\');
                }
                bytes.push(b);
            } else {
                bytes.push(match b {
                    b'a' => b'\x07',
                    b'b' => b'\x08',
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => b'\x0b',
                    b'f' => b'\x0c',
                    b'r' => b'\r',
                    b'e' => b'\x1b',
                    b's' => b' ',
                    b => b,
                });
            }
            escaping = false;
        } else if b == b'\\' {
            escaping = true;
        } else {
            bytes.push(b);
        }
    }
    bytes
}
