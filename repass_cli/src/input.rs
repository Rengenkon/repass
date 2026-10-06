use std::io::{self, BufRead, Read};
use std::path::Path;

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

pub fn read_line(input: &mut impl BufRead, limit: usize) -> io::Result<String> {
    let mut bytes = Vec::new();
    input.take(limit as u64 + 1).read_until(b'\n', &mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input exceeds size limit",
        ));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn read_text(input: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_TEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "text exceeds 1 MiB size limit",
        ));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn read_file(path: &Path) -> io::Result<String> {
    read_text(&mut std::fs::File::open(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bounded_reads_stop_after_the_limit_without_consuming_the_whole_stream() {
        let mut input = Cursor::new(b"abcdef\nnext\n");
        assert!(read_line(&mut input, 3).is_err());
        assert_eq!(input.position(), 4);
        assert_eq!(
            read_line(&mut Cursor::new("\u{1f980}\n"), 5).unwrap(),
            "\u{1f980}\n"
        );
        assert!(read_line(&mut Cursor::new("\u{1f980}\n"), 4).is_err());
        assert_eq!(read_text(&mut Cursor::new("key\r\n")).unwrap(), "key\r\n");
        assert!(read_text(&mut io::repeat(b'x')).is_err());
    }
}
