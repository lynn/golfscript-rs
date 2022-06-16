pub fn unescape(lexeme: &[u8], single_quoted: bool) -> Vec<u8> {
    let mut bytes = vec![];
    let mut escaping = false;
    for i in 1..lexeme.len() - 1 {
        if escaping {
            if single_quoted {
                if lexeme[i] != b'\\' && lexeme[i] != b'\'' {
                    bytes.push(b'\\');
                }
                bytes.push(lexeme[i]);
            } else {
                bytes.push(match lexeme[i] {
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
        } else if lexeme[i] == b'\\' {
            escaping = true;
        } else {
            bytes.push(lexeme[i]);
        }
    }
    bytes
}
