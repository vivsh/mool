/// Output of PlaceholderIter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderPart<'a> {
    Sql(&'a str),
    Placeholder(&'a str), // name WITHOUT ':'
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    SingleQuote,
    DoubleQuote,
    Backtick,
    BracketIdent,
    LineComment,
    BlockComment,
    DollarQuote { tag_start: usize, tag_end: usize }, // tag is s[tag_start..tag_end], may be empty
}

/// Non-allocating, panic-free iterator over SQL + :name placeholders.
/// - Only detects ASCII placeholders: :[A-Za-z_][A-Za-z0-9_]*
/// - Skips: strings, quoted identifiers, line/block comments, PG dollar quotes ($$ or $tag$)
pub struct PlaceholderIter<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize, // scan cursor (byte index)
    chunk_start: usize,
    mode: Mode,
    done: bool,
}

impl<'a> PlaceholderIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self {
            s,
            b: s.as_bytes(),
            i: 0,
            chunk_start: 0,
            mode: Mode::Normal,
            done: false,
        }
    }

    #[inline]
    fn is_name_start(x: u8) -> bool {
        x.is_ascii_lowercase() || x.is_ascii_uppercase() || x == b'_'
    }
    #[inline]
    fn is_name_char(x: u8) -> bool {
        Self::is_name_start(x) || x.is_ascii_digit()
    }
    #[inline]
    fn is_word_char(x: u8) -> bool {
        Self::is_name_char(x)
    }

    #[inline]
    fn slice(&self, a: usize, b: usize) -> &'a str {
        // Safe because we only advance on UTF-8 character boundaries.
        // Indices are always in-range by construction (checked before advancing).
        &self.s[a..b]
    }

    /// Advance by one UTF-8 character. Returns the byte length of the character.
    /// This ensures we never land in the middle of a multi-byte character.
    /// Returns 0 if at end of string. Steps 1 byte on invalid UTF-8.
    #[inline]
    fn advance_one_char(&mut self) -> usize {
        let Some(&b0) = self.b.get(self.i) else {
            return 0;
        };

        // Fast path for ASCII (vast majority of SQL)
        if b0 < 0x80 {
            self.i += 1;
            return 1;
        }

        // Determine UTF-8 character length from leading byte
        let len = if (b0 & 0b1110_0000) == 0b1100_0000 {
            2
        } else if (b0 & 0b1111_0000) == 0b1110_0000 {
            3
        } else if (b0 & 0b1111_1000) == 0b1111_0000 {
            4
        } else {
            1 // Invalid lead byte, step 1
        };

        let end = self.i.saturating_add(len).min(self.b.len());
        // Validate continuation bytes (must match 0b10xx_xxxx pattern)
        if end - self.i == len
            && self.b[self.i + 1..end]
                .iter()
                .all(|&c| (c & 0b1100_0000) == 0b1000_0000)
        {
            self.i = end;
            len
        } else {
            // Invalid UTF-8 sequence, step 1 byte
            self.i += 1;
            1
        }
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    #[inline]
    fn peek_n(&self, n: usize) -> Option<u8> {
        self.b.get(self.i + n).copied()
    }

    #[inline]
    fn starts_with_at(&self, pos: usize, pat: &[u8]) -> bool {
        self.b.get(pos..pos + pat.len()) == Some(pat)
    }

    // Attempts to parse a dollar-quote opener at current `i` (which must be on '$').
    // On success, sets mode and advances `i` past opener, returning true.
    fn try_enter_dollar_quote(&mut self) -> bool {
        // at '$'
        // $$ ...
        if self.starts_with_at(self.i, b"$$") {
            self.mode = Mode::DollarQuote {
                tag_start: self.i + 1,
                tag_end: self.i + 1, // empty tag
            };
            self.advance_one_char(); // first $
            self.advance_one_char(); // second $
            return true;
        }

        // $tag$
        let tag_start = self.i + 1;
        let first = self.b.get(tag_start).copied();
        if first.is_none_or(|b| !Self::is_name_start(b)) {
            return false;
        }

        let mut j = tag_start + 1;
        while let Some(&ch) = self.b.get(j) {
            if Self::is_name_char(ch) {
                j += 1;
            } else {
                break;
            }
        }
        // must end with '$'
        if self.b.get(j).copied() == Some(b'$') {
            self.mode = Mode::DollarQuote {
                tag_start,
                tag_end: j, // excludes '$'
            };
            // Advance past $ + tag + $
            // Tag is ASCII (alphanumeric/underscore) so byte-wise is safe
            self.i = j + 1;
            return true;
        }

        false
    }

    // Attempts to detect dollar-quote closer at current `i` (which must be on '$').
    // On success, consumes closer and returns true.
    fn try_exit_dollar_quote(&mut self) -> bool {
        let (tag_start, tag_end) = match self.mode {
            Mode::DollarQuote { tag_start, tag_end } => (tag_start, tag_end),
            _ => return false,
        };

        // Empty tag: "$$"
        if tag_start == tag_end {
            if self.starts_with_at(self.i, b"$$") {
                self.advance_one_char(); // first $
                self.advance_one_char(); // second $
                self.mode = Mode::Normal;
                return true;
            }
            return false;
        }

        // Non-empty: "$tag$"
        // Need: '$' + tag + '$'
        let tag = self.b.get(tag_start..tag_end).unwrap_or(&[]);
        if self.peek() != Some(b'$') {
            return false;
        }
        let after_dollar = self.i + 1;
        if self.b.get(after_dollar..after_dollar + tag.len()) != Some(tag) {
            return false;
        }
        if self.b.get(after_dollar + tag.len()).copied() != Some(b'$') {
            return false;
        }

        // Advance past the closing delimiter: $ + tag + $
        self.advance_one_char(); // opening $
        // Tag is ASCII (alphanumeric/underscore), so byte-wise advancement is safe
        self.i += tag.len();
        self.advance_one_char(); // closing $
        self.mode = Mode::Normal;
        true
    }
}

impl<'a> Iterator for PlaceholderIter<'a> {
    type Item = PlaceholderPart<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        while self.i < self.b.len() {
            match self.mode {
                Mode::Normal => {
                    let c = self.b[self.i];

                    // -- line comment
                    if c == b'-' && self.peek_n(1) == Some(b'-') {
                        self.advance_one_char(); // first -
                        self.advance_one_char(); // second -
                        self.mode = Mode::LineComment;
                        continue;
                    }
                    // # line comment
                    if c == b'#' {
                        self.advance_one_char();
                        self.mode = Mode::LineComment;
                        continue;
                    }
                    // /* block comment */
                    if c == b'/' && self.peek_n(1) == Some(b'*') {
                        self.advance_one_char(); // /
                        self.advance_one_char(); // *
                        self.mode = Mode::BlockComment;
                        continue;
                    }

                    // quotes / identifiers
                    if c == b'\'' {
                        self.advance_one_char();
                        self.mode = Mode::SingleQuote;
                        continue;
                    }
                    if c == b'"' {
                        self.advance_one_char();
                        self.mode = Mode::DoubleQuote;
                        continue;
                    }
                    if c == b'`' {
                        self.advance_one_char();
                        self.mode = Mode::Backtick;
                        continue;
                    }
                    if c == b'[' {
                        self.advance_one_char();
                        self.mode = Mode::BracketIdent;
                        continue;
                    }

                    // dollar quote
                    if c == b'$' {
                        if self.try_enter_dollar_quote() {
                            continue;
                        }
                        self.advance_one_char();
                        continue;
                    }

                    // placeholder :name (NOT :: and NOT abc:name)
                    if c == b':' {
                        let prev = if self.i == 0 {
                            None
                        } else {
                            Some(self.b[self.i - 1])
                        };
                        // Only reject if prev is a word char AND we didn't just emit a placeholder
                        let prev_is_word = prev.map(Self::is_word_char).unwrap_or(false);
                        let prev_is_colon = prev == Some(b':');
                        let just_emitted_placeholder = self.chunk_start == self.i;

                        if !prev_is_colon
                            && (!prev_is_word || just_emitted_placeholder)
                            && let Some(n0) = self.peek_n(1)
                            && Self::is_name_start(n0)
                        {
                            let name_start = self.i + 1;
                            let mut j = name_start + 1;
                            while let Some(&ch) = self.b.get(j) {
                                if Self::is_name_char(ch) {
                                    j += 1;
                                } else {
                                    break;
                                }
                            }

                            // Emit SQL chunk before placeholder, then placeholder in next call.
                            if self.chunk_start < self.i {
                                let sql = self.slice(self.chunk_start, self.i);
                                self.chunk_start = self.i; // placeholder begins here
                                return Some(PlaceholderPart::Sql(sql));
                            }

                            // Emit placeholder (name only), advance past it.
                            let name = self.slice(name_start, j);
                            self.i = j;
                            self.chunk_start = j;
                            return Some(PlaceholderPart::Placeholder(name));
                        }

                        self.advance_one_char();
                        continue;
                    }

                    self.advance_one_char();
                }

                Mode::SingleQuote => {
                    match self.peek() {
                        Some(b'\\') => {
                            // backslash: skip it and next char if present
                            self.advance_one_char();
                            if self.peek().is_some() {
                                self.advance_one_char();
                            }
                        }
                        Some(b'\'') => {
                            if self.peek_n(1) == Some(b'\'') {
                                self.advance_one_char(); // first '
                                self.advance_one_char(); // second '
                            } else {
                                self.advance_one_char();
                                self.mode = Mode::Normal;
                            }
                        }
                        _ => {
                            self.advance_one_char();
                        }
                    }
                }

                Mode::DoubleQuote => {
                    match self.peek() {
                        Some(b'\\') => {
                            // backslash: skip it and next char if present
                            self.advance_one_char();
                            if self.peek().is_some() {
                                self.advance_one_char();
                            }
                        }
                        Some(b'"') => {
                            if self.peek_n(1) == Some(b'"') {
                                self.advance_one_char(); // first "
                                self.advance_one_char(); // second "
                            } else {
                                self.advance_one_char();
                                self.mode = Mode::Normal;
                            }
                        }
                        _ => {
                            self.advance_one_char();
                        }
                    }
                }

                Mode::Backtick => {
                    match self.peek() {
                        Some(b'\\') => {
                            // backslash: skip it and next char if present
                            self.advance_one_char();
                            if self.peek().is_some() {
                                self.advance_one_char();
                            }
                        }
                        Some(b'`') => {
                            if self.peek_n(1) == Some(b'`') {
                                self.advance_one_char(); // first `
                                self.advance_one_char(); // second `
                            } else {
                                self.advance_one_char();
                                self.mode = Mode::Normal;
                            }
                        }
                        _ => {
                            self.advance_one_char();
                        }
                    }
                }

                Mode::BracketIdent => {
                    if self.peek() == Some(b']') {
                        self.advance_one_char();
                        self.mode = Mode::Normal;
                    } else {
                        self.advance_one_char();
                    }
                }

                Mode::LineComment => {
                    if self.peek() == Some(b'\n') {
                        self.advance_one_char();
                        self.mode = Mode::Normal;
                    } else {
                        self.advance_one_char();
                    }
                }

                Mode::BlockComment => {
                    if self.peek() == Some(b'*') && self.peek_n(1) == Some(b'/') {
                        self.advance_one_char(); // *
                        self.advance_one_char(); // /
                        self.mode = Mode::Normal;
                    } else {
                        self.advance_one_char();
                    }
                }

                Mode::DollarQuote { .. } => {
                    // scan until we see a '$' that matches closer
                    if self.peek() == Some(b'$') && self.try_exit_dollar_quote() {
                        continue;
                    }
                    self.advance_one_char();
                }
            }
        }

        // end: flush remaining SQL chunk
        self.done = true;
        if self.chunk_start < self.s.len() {
            Some(PlaceholderPart::Sql(
                self.slice(self.chunk_start, self.s.len()),
            ))
        } else {
            None
        }
    }
}
