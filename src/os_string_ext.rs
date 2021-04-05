use std::ffi::OsString;
#[cfg(windows)] use std::os::windows::ffi::OsStrExt;

pub struct OsSplitIter {
    #[cfg(not(windows))] separator: u8,
    #[cfg(windows)] separator: u16,
    i: usize,
    #[cfg(not(windows))] vec: Vec<u8>,
    #[cfg(windows)] vec: Vec<u16>,
}

pub struct OsWhitespaceSplitIter {
    i: usize,
    #[cfg(not(windows))] vec: Vec<u8>,
    #[cfg(windows)] vec: Vec<u16>,
}

pub trait OsStringExt {
    #[cfg(windows)] fn replace_str(&self, from: &OsString, to: &OsString) -> OsString;
    #[cfg(not(windows))] fn replace_str(&self, from: &[u8], to: &[u8]) -> OsString;
    fn replace(&self, from: u8, to: u8) -> OsString;
    fn split(&self, separator: u8) -> OsSplitIter;
    fn split_at(&self, index: usize) -> (OsString, OsString);
    fn split_ascii_whitespace(&self) -> OsWhitespaceSplitIter;
    fn starts_with(&self, string: OsString) -> bool;
}

impl OsStringExt for OsString {
    // Note: when no replacement this still constructs a new OsString from the original.
    fn replace_str(&self, from: &OsString, to: &OsString) -> OsString {
        use std::os::windows::ffi::OsStringExt;
        let vec: Vec<u16> = self.encode_wide().collect();
        let from: Vec<u16> = from.encode_wide().collect();
        let to: Vec<u16> = to.encode_wide().collect();
        let mut new: Vec<u16> = vec![];
        let mut last = 0;
        for i in vec
            .windows(from.len())
            .enumerate()
            .filter_map(|(i, b)| {
                if b == from {
                    Some(i)
                } else {
                    None
                }
            }) {
            new.append(&mut vec[last..i].to_vec());
            let mut tto = to.clone();
            new.append(&mut tto);
            last = i + from.len();
        }
        new.append(&mut vec[last..].to_vec());

        OsString::from_wide(&new)
    }

    #[cfg(windows)]
    fn replace(&self, from: u8, to: u8) -> Self {
        use std::os::windows::ffi::OsStringExt;

        let from = u16::from(from);
        let to = u16::from(to);
        let vec: Vec<u16> = self.encode_wide().map(|b| if b == from { to } else { b } ).collect();

        OsString::from_wide(&vec)
    }

    #[cfg(windows)]
    fn split(&self, separator: u8) -> OsSplitIter {
        OsSplitIter {
            separator: u16::from(separator),
            i: 0,
            vec: self.encode_wide().collect(),
        }
    }

    #[cfg(not(windows))]
    fn split(&self, separator: u8) -> OsSplitIter {
        OsSplitIter {
            separator,
            i: 0,
            vec: self.iter().collect(),
        }
    }

    #[cfg(windows)]
    fn split_ascii_whitespace(&self) -> OsWhitespaceSplitIter {
        OsWhitespaceSplitIter {
            i: 0,
            vec: self.encode_wide().collect(),
        }
    }

    #[cfg(windows)]
    fn split_at(&self, index: usize) -> (OsString, OsString) {
        use std::os::windows::ffi::OsStringExt;
        let vec: Vec<u16> = self.encode_wide().collect();

        if index >= vec.len() {
            (self.clone(), OsString::new())
        } else {
            (OsString::from_wide(&vec[0..index]), OsString::from_wide(&vec[index..vec.len()]))
        }
    }

    #[cfg(not(windows))]
    fn split_at(&self, index: usize) -> (OsString, OsString) {
        let vec: Vec<u16> = self.iter().collect();

        if index >= vec.len() {
            (self.clone(), OsString::new())
        } else {
            (OsString::new(&vec[0..index]), OsString::new(&vec[index..vec.len()]))
        }
    }

    #[cfg(windows)]
    fn starts_with(&self, string: OsString) -> bool {
        let vec: Vec<u16> = self.encode_wide().collect();
        let start_vec: Vec<u16> = string.encode_wide().collect();

        vec.starts_with(start_vec.as_slice())
    }

    #[cfg(not(windows))]
    fn starts_with(&self, string: OsString) -> bool {
        let vec: Vec<u8> = self.iter().collect();
        let start_vec: Vec<u8> = string.iter().collect();

        vec.starts_with(start_vec.as_slice())
    }
}

#[cfg(windows)] const SPACE: u16 = b' ' as u16;
#[cfg(windows)] const RETURN: u16 = b'\r' as u16;
#[cfg(windows)] const TAB: u16 = b'\t' as u16;
#[cfg(windows)] const NEWLINE: u16 = b'\n' as u16;
#[cfg(windows)] const LINEFEED: u16 = b'\x0C' as u16;
#[cfg(not(windows))] const SPACE: u16 = b' ' as u16;
#[cfg(not(windows))] const RETURN: u16 = b'\r' as u16;
#[cfg(not(windows))] const TAB: u16 = b'\t' as u16;
#[cfg(not(windows))] const NEWLINE: u16 = b'\n' as u16;
#[cfg(not(windows))] const LINEFEED: u16 = b'\x0C' as u16;

impl Iterator for OsWhitespaceSplitIter {
    type Item = OsString;

    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(windows)]
        fn result(vec: &Vec<u16>, start_index: usize, end_index: usize) -> Option<OsString> {
            use std::os::windows::ffi::OsStringExt;
            Some(OsString::from_wide(&vec[start_index..end_index]))
        }

        #[cfg(not(windows))]
        fn result(vec: &Vec<u8>, start_index: usize, end_index: usize) -> Option<OsString> {
            Some(OsString::new(&vec[start_index..end_index]))
        }

        #[cfg(windows)]
        fn is_whitespace(b: &u16) -> bool {
            matches!(*b, TAB | NEWLINE | LINEFEED | RETURN | SPACE)
        }

        #[cfg(not(windows))]
        fn is_whitespace(b: &u8) -> bool {
            matches!(*b, TAB | NEWLINE | LINEFEED | RETURN | SPACE)
        }

        let length = self.vec.len();
        if self.i >= length {
            return None;
        }

        let mut start_index = self.i;
        let mut end_index = start_index;

       self
           .vec
           .iter()
           .enumerate()
           .skip(self.i)
           .skip_while(|(i, b)| {
               start_index = *i;
               is_whitespace(b)
           }).skip_while(|(i, b)| {
               end_index = *i;
               !is_whitespace(b)
           }).for_each(drop);

        if start_index + 1 >= length {         // \s+$
            return None
        } else if end_index + 1 >= length {   //  \S+$
            end_index = self.vec.len();
        }

        self.i = end_index + 1;

        result(&self.vec, start_index, end_index)
    }
}

impl Iterator for OsSplitIter {
    type Item = OsString;

    fn next(&mut self) -> Option<Self::Item> {
        #[cfg(windows)]
        fn result(vec: &Vec<u16>, start_index: usize, end_index: usize) -> Option<OsString> {
            use std::os::windows::ffi::OsStringExt;
            Some(OsString::from_wide(&vec[start_index..end_index]))
        }

        #[cfg(not(windows))]
        fn result(vec: &Vec<u8>, start_index: usize, end_index: usize) -> Option<OsString> {
            Some(OsString::new(&vec[start_index..end_index]))
        }

        if self.i >= self.vec.len() {
            return None;
        }

        let start_index = self.i;
        let offset = self.vec.iter().skip(self.i).position(|b| *b == self.separator);
        let end_index = match offset {
            Some(offset) => self.i + offset,
            None => self.vec.len(),
        };

        self.i = end_index + 1;

        result(&self.vec, start_index, end_index)
    }
}

#[cfg(test)]
mod tests {
    use crate::os_string_ext::OsStringExt;
    use std::ffi::OsString;

    #[test]
    fn replace_none() {
        assert_eq!(OsString::from(""), "");
        assert_eq!(OsString::from("onions"), "onions");
    }

    #[test]
    fn replace_simple() {
        assert_eq!(OsString::from("My.potato.and.onions").replace(b'.', b'/'),
                   "My/potato/and/onions");
        assert_eq!(OsString::from(".My.potato.and.onions.").replace(b'.', b'/'),
                   "/My/potato/and/onions/");
    }

    #[test]
    fn replace_str_none() {
        let orig = OsString::from("My.potato.and.onions");
        assert_eq!(orig.replace_str(&OsString::from("zoo"), &OsString::from("carrots")), orig);
    }

    #[test]
    fn replace_str_simple() {
        let orig = OsString::from("My.potato.and.onions");
        assert_eq!(orig.replace_str(&OsString::from("onions"), &OsString::from("carrots")),
                   "My.potato.and.carrots");
        let orig = OsString::from("My.potato.and.onions.and.onions");
        assert_eq!(orig.replace_str(&OsString::from("onions"), &OsString::from("carrots")),
                   "My.potato.and.carrots.and.carrots");
    }

    #[test]
    fn split_ascii_whitespace_none() {
        let mut i = OsString::from("").split_ascii_whitespace();
        assert!(i.next().is_none());
        let mut i = OsString::from("onelongnospace").split_ascii_whitespace();
        assert_eq!(i.next().unwrap(), "onelongnospace");
    }

    #[test]
    fn split_ascii_whitespace_simple() {
        let mut i = OsString::from("My \t  potato\r\nand onions").split_ascii_whitespace();
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert_eq!(i.next().unwrap(), "and");
        assert_eq!(i.next().unwrap(), "onions");
        assert!(i.next().is_none());

        let mut i = OsString::from("  My \t  potato\r\nand onions").split_ascii_whitespace();
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert_eq!(i.next().unwrap(), "and");
        assert_eq!(i.next().unwrap(), "onions");
        assert!(i.next().is_none());

        let mut i = OsString::from("  My \t  potato\r\nand onions  ").split_ascii_whitespace();
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert_eq!(i.next().unwrap(), "and");
        assert_eq!(i.next().unwrap(), "onions");
        assert!(i.next().is_none());
    }

    #[test]
    fn split_simple() {
        let mut i = OsString::from("My potato").split(b' ');
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert!(i.next().is_none());
    }

    #[test]
    fn split_simple_last_split_byte() {
        let mut i = OsString::from("My potato ").split(b' ');
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert!(i.next().is_none());
    }

    #[test]
    fn split_simple_first_split_byte() {
        let mut i = OsString::from(" My potato").split(b' ');
        assert_eq!(i.next().unwrap(), "");
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "potato");
        assert!(i.next().is_none());
    }

    #[test]
    fn split_simple_multiple_split_bytes_in_a_row() {
        let mut i = OsString::from("My   potato ").split(b' ');
        assert_eq!(i.next().unwrap(), "My");
        assert_eq!(i.next().unwrap(), "");
        assert_eq!(i.next().unwrap(), "");
        assert_eq!(i.next().unwrap(), "potato");
        assert!(i.next().is_none());
    }

    #[test]
    fn split_not_found() {
        let mut i = OsString::from("My potato").split(b'#');
        assert_eq!(i.next().unwrap(), "My potato");
    }

    #[test]
    fn split_on_empty() {
        let mut i = OsString::from("").split(b'#');
        assert!(i.next().is_none())
    }

    #[test]
    fn split_at_simple() {
        let (left, right) = OsString::from("-Xpotato").split_at(2);

        assert_eq!(left, "-X");
        assert_eq!(right, "potato");
    }

    #[test]
    fn split_at_too_large() {
        let (left, right) = OsString::from("-Xpotato").split_at(200);

        assert_eq!(left, "-Xpotato");
        assert_eq!(right, "");
    }

    #[test]
    fn starts_with_simple() {
        assert!(OsString::from("-Xpotato").starts_with(OsString::from("-X")));
        assert_eq!(false, OsString::from("-Xpotato").starts_with(OsString::from("-D")));
    }
}