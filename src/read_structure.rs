//! Read Structures
//!
//! Type [`ReadStructure`] describes the structure of a given read.  A read
//! contains one or more read segments. A read segment describes a contiguous
//! stretch of bases of the same type (e.g. template bases).
//!
//! At most one segment may be the indefinite-length (`+`) segment meaning "the rest
//! of the read"; it can appear in any position, not just the terminal one.

use crate::ErrorMessageParts;
use crate::ReadStructureError;
use crate::read_segment::ANY_LENGTH_BYTE;
use crate::read_segment::ReadSegment;
use crate::segment_type::SegmentType;
use std::ops::Index;

/// A read structure made up of one or more [`ReadSegment`]s.
///
/// Internally, in addition to the segments themselves, we cache per-segment start
/// offsets (signed) and enough summary information to resolve the indefinite-length
/// (`+`) segment's end position at extract time without another pass over the
/// segments.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReadStructure {
    /// The elements that make up the [`ReadStructure`].
    elements: Vec<ReadSegment>,
    /// Sum of lengths across all fixed-length (non-`+`) segments.
    length_of_fixed_segments: usize,
    /// Index of the indefinite-length (`+`) segment, if any.
    plus_index: Option<usize>,
    /// Sum of fixed-length-segment lengths strictly AFTER the `+` segment. Zero when
    /// there is no `+` or when `+` is terminal. Allows the end of the `+` segment to
    /// be located as `read_len - post_plus_len` at extract time.
    post_plus_len: usize,
    /// Per-element start offset.
    ///
    /// Sign carries the frame of reference:
    /// - `>= 0` — offset from the start of the read. Used for segments before or
    ///   at the `+`, and for every segment when there is no `+`.
    /// - `<  0` — distance from the end of the read, stored as a negative number.
    ///   Used for segments strictly after a non-terminal `+`. The actual start
    ///   position in a read of length `L` is `L + offset` (i.e., `L - (-offset)`).
    offsets: Vec<isize>,
}

impl ReadStructure {
    /// Builds a new [`ReadStructure`] from a vector of [`ReadSegment`]s.
    ///
    /// At most one segment may have an indefinite length (`+`); it may appear in any
    /// position.
    ///
    /// # Errors
    ///
    /// - Returns `Err` if no elements exist.
    /// - Returns `Err` if more than one segment has an indefinite length.
    #[allow(clippy::missing_panics_doc)]
    pub fn new(segments: Vec<ReadSegment>) -> Result<Self, ReadStructureError> {
        if segments.is_empty() {
            return Err(ReadStructureError::ReadStructureContainsZeroElements);
        }

        let mut num_indefinite = 0;
        let mut length_of_fixed_segments = 0;
        let mut plus_index: Option<usize> = None;
        for (i, s) in segments.iter().enumerate() {
            if let Some(len) = s.length {
                length_of_fixed_segments += len;
            } else {
                num_indefinite += 1;
                if plus_index.is_none() {
                    plus_index = Some(i);
                }
            }
        }

        if num_indefinite > 1 {
            return Err(ReadStructureError::ReadStructureNonTerminalIndefiniteLengthReadSegment(
                *segments.iter().find(|s| !s.has_length()).unwrap(),
            ));
        }

        // Compute per-element offsets. Two-pass: forward for pre-and-at-`+` (and
        // for every segment when there is no `+`), then backward for segments
        // strictly after the `+`.
        let n = segments.len();
        let mut offsets = vec![0isize; n];

        // Forward pass up to and including the `+` (or the whole vec if no `+`).
        let forward_end = plus_index.map_or(n, |p| p + 1);
        let mut off: usize = 0;
        for (i, seg) in segments.iter().take(forward_end).enumerate() {
            offsets[i] = off as isize;
            off += seg.length.unwrap_or(0);
        }

        // Backward pass for segments strictly after the `+`, if any.
        let mut post_plus_len: usize = 0;
        if let Some(p) = plus_index {
            // `dist_from_end` is the total number of bases from the *start* of the
            // current segment to the end of the read. We walk from the end back.
            let mut dist_from_end: usize = 0;
            for (i, seg) in segments.iter().enumerate().skip(p + 1).rev() {
                // Fixed length — invariants: non-`+` segments always have a length.
                let len = seg.length.expect("post-+ segments must be fixed length");
                dist_from_end += len;
                offsets[i] = -(dist_from_end as isize);
            }
            post_plus_len = dist_from_end;
        }

        Ok(ReadStructure {
            elements: segments,
            length_of_fixed_segments,
            plus_index,
            post_plus_len,
            offsets,
        })
    }

    /// Extracts the bases and quality scores for every segment in this read structure.
    ///
    /// Returns a no-alloc iterator over `(&ReadSegment, &[u8] bases, &[u8] quals)` triples.
    /// All error conditions are checked up front, so the returned iterator is infallible
    /// and can be cheaply `.collect()`ed into a `Vec` when an owned collection is needed.
    ///
    /// When `include_skips` is `false`, segments of type [`SegmentType::Skip`] are
    /// omitted from the output.
    ///
    /// # Errors
    ///
    /// - [`ReadStructureError::MismatchingBasesAndQualsLen`] if `bases.len() != quals.len()`.
    /// - [`ReadStructureError::ReadTooShort`] if the read cannot accommodate every fixed
    ///   segment. When the structure also has a `+` segment, the read must be strictly
    ///   longer than the sum of the fixed-segment lengths (since `+` means at least
    ///   one base).
    pub fn extract<'rs, 'b>(
        &'rs self,
        bases: &'b [u8],
        quals: &'b [u8],
        include_skips: bool,
    ) -> Result<ExtractedSegments<'rs, 'b>, ReadStructureError> {
        if bases.len() != quals.len() {
            return Err(ReadStructureError::MismatchingBasesAndQualsLen {
                bases_len: bases.len(),
                quals_len: quals.len(),
            });
        }

        let required = if self.plus_index.is_some() {
            self.length_of_fixed_segments + 1
        } else {
            self.length_of_fixed_segments
        };
        if bases.len() < required {
            return Err(ReadStructureError::ReadTooShort { read_len: bases.len(), required });
        }

        Ok(ExtractedSegments {
            elements: &self.elements,
            offsets: &self.offsets,
            plus_index: self.plus_index,
            post_plus_len: self.post_plus_len,
            bases,
            quals,
            include_skips,
            next_index: 0,
        })
    }

    /// Returns `true` if the [`ReadStructure`] has a fixed (i.e. non-variable) length,
    /// `false` if any segment has an indefinite length.
    pub fn has_fixed_length(&self) -> bool {
        self.plus_index.is_none()
    }

    /// Returns the fixed length if there is one.
    pub fn fixed_length(&self) -> Option<usize> {
        if self.has_fixed_length() { Some(self.length_of_fixed_segments) } else { None }
    }

    /// Returns the number of segments in this read structure.
    pub fn number_of_segments(&self) -> usize {
        self.elements.len()
    }

    /// Returns the underlying elements in this read structure.
    pub fn segments(&self) -> &[ReadSegment] {
        &self.elements
    }

    /// Returns an iterator over the read segments.
    pub fn iter(&self) -> impl Iterator<Item = &ReadSegment> {
        self.elements.iter()
    }

    /// Returns the [`ReadSegment`]s in this read structure of the given kind.
    pub fn segments_by_type(&self, kind: SegmentType) -> impl Iterator<Item = &ReadSegment> {
        self.elements.iter().filter(move |seg| seg.kind == kind)
    }

    /// Returns the template [`ReadSegment`]s in this read structure.
    pub fn templates(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::Template)
    }

    /// Returns the sample barcode [`ReadSegment`]s in this read structure.
    pub fn sample_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::SampleBarcode)
    }

    /// Returns the molecular barcode [`ReadSegment`]s in this read structure.
    pub fn molecular_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::MolecularBarcode)
    }

    /// Returns the skip [`ReadSegment`]s in this read structure.
    pub fn skips(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::Skip)
    }

    /// Returns the cellular barcode [`ReadSegment`]s in this read structure.
    pub fn cellular_barcodes(&self) -> impl Iterator<Item = &ReadSegment> {
        self.segments_by_type(SegmentType::CellularBarcode)
    }

    /// Returns the first [`ReadSegment`] in this read structure.
    pub fn first(&self) -> Option<&ReadSegment> {
        self.elements.first()
    }

    /// Returns the last [`ReadSegment`] in this read structure.
    pub fn last(&self) -> Option<&ReadSegment> {
        self.elements.last()
    }
}

/// Iterator returned by [`ReadStructure::extract`].
///
/// Yields `(&ReadSegment, &[u8] bases, &[u8] quals)` triples — one per segment in
/// the underlying read structure, in order (with `Skip` segments optionally filtered).
/// The iterator is infallible: all error checks are performed up front in
/// [`ReadStructure::extract`].
#[derive(Debug)]
pub struct ExtractedSegments<'rs, 'b> {
    elements: &'rs [ReadSegment],
    offsets: &'rs [isize],
    plus_index: Option<usize>,
    post_plus_len: usize,
    bases: &'b [u8],
    quals: &'b [u8],
    include_skips: bool,
    next_index: usize,
}

impl<'rs, 'b> Iterator for ExtractedSegments<'rs, 'b> {
    type Item = (&'rs ReadSegment, &'b [u8], &'b [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        while self.next_index < self.elements.len() {
            let i = self.next_index;
            self.next_index += 1;
            let seg = &self.elements[i];
            if !self.include_skips && seg.kind == SegmentType::Skip {
                continue;
            }
            let (start, end) = if Some(i) == self.plus_index {
                // The indefinite-length segment: runs from its stored start offset
                // to just before the post-`+` fixed region.
                (self.offsets[i] as usize, self.bases.len() - self.post_plus_len)
            } else {
                let off = self.offsets[i];
                let start =
                    if off >= 0 { off as usize } else { self.bases.len() - ((-off) as usize) };
                // Non-`+` segments always have a fixed length.
                let len = seg.length.expect("non-`+` segment must have a length");
                (start, start + len)
            };
            return Some((seg, &self.bases[start..end], &self.quals[start..end]));
        }
        None
    }
}

impl IntoIterator for ReadStructure {
    type Item = ReadSegment;

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl Index<usize> for ReadStructure {
    type Output = ReadSegment;

    /// Returns the [`ReadSegment`] at the given index in the read structure.
    fn index(&self, idx: usize) -> &Self::Output {
        &self.elements[idx]
    }
}

impl std::fmt::Display for ReadStructure {
    /// Formats this read structure as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.elements {
            write!(f, "{}", e)?;
        }
        Ok(())
    }
}

impl std::str::FromStr for ReadStructure {
    type Err = ReadStructureError;

    /// Returns a new read structure from a string, or `Err` if parsing failed.
    fn from_str(rs: &str) -> Result<Self, Self::Err> {
        let mut i = 0;
        let mut segs: Vec<ReadSegment> = Vec::new();
        let chars: Vec<char> = rs.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
        while i < chars.len() {
            // Stash the beginning position of our parsing so we can highlight what we're having trouble with
            let parse_i = i;

            // Parse out the length segment which many be 1 or more digits or the AnyLengthChar
            let length = if chars[i] as u8 == ANY_LENGTH_BYTE {
                i += 1;
                None
            } else if chars[i].is_ascii_digit() {
                let mut len: usize = 0;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    // Unwrap is safe since we've checked `is_ascii_digit` already
                    let digit = chars[i].to_digit(10).unwrap() as usize;
                    len = (len * 10) + digit;
                    i += 1;
                }
                Some(len)
            } else {
                return Err(ReadStructureError::ReadStructureMissingLengthInformation(
                    ErrorMessageParts::new(&chars, parse_i, parse_i + 1),
                ));
            };

            // Parse out the operator and make a segment
            if chars.len() == i {
                return Err(ReadStructureError::ReadStructureMissingOperator(
                    ErrorMessageParts::new(&chars, parse_i, i),
                ));
            } else if let Ok(kind) = SegmentType::try_from(chars[i]) {
                if length == Some(0) {
                    return Err(ReadStructureError::ReadSegmentLengthZero(ErrorMessageParts::new(
                        &chars, parse_i, i,
                    )));
                }
                i += 1;
                segs.push(ReadSegment { length, kind });
            } else {
                return Err(ReadStructureError::ReadStructureHadUnknownType(
                    ErrorMessageParts::new(&chars, parse_i, i + 1),
                ));
            }
        }

        ReadStructure::new(segs)
    }
}

impl TryFrom<&[ReadSegment]> for ReadStructure {
    type Error = ReadStructureError;
    /// Builds a new read structure from a slice of elements.
    fn try_from(elements: &[ReadSegment]) -> Result<Self, Self::Error> {
        Self::new(elements.to_vec())
    }
}

#[cfg(test)]
mod test {
    use crate::ReadStructureError;
    use crate::read_structure::ReadStructure;
    use crate::segment_type::SegmentType;
    use std::str::FromStr;

    #[test]
    fn test_read_structure_from_str() {
        let rss =
            ["1T", "1B", "1M", "1S", "101T", "5B101T", "123456789T", "10T10B10B10S10M", "5B2C3T"];
        for rs in &rss {
            assert_eq!(ReadStructure::from_str(rs).unwrap().to_string(), *rs);
        }
    }

    #[test]
    fn test_read_structure_from_str_with_whitespace() {
        let rss = ["75T 8B 8B 75T", " 75T 8B 8B\t75T  "];
        for rs in &rss {
            assert_eq!(ReadStructure::from_str(rs).unwrap().to_string(), "75T8B8B75T");
        }
    }

    #[test]
    fn test_read_structure_allow_anylength_char_only_once_and_for_last_segment() {
        assert_eq!(ReadStructure::from_str("5M+T").unwrap().to_string(), "5M+T");
        assert_eq!(ReadStructure::from_str("+M").unwrap().to_string(), "+M");
    }

    macro_rules! test_read_structure_from_str_err {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                 assert!(ReadStructure::from_str($value).is_err());
            }
        )*
        }
    }

    test_read_structure_from_str_err! {
        test_read_structure_rejects_multiple_plus_0: "++M",
        test_read_structure_rejects_multiple_plus_1: "5M++T",
        test_read_structure_rejects_multiple_plus_2: "5M70+T",
        test_read_structure_rejects_multiple_plus_3: "+M+T",
        test_read_structure_rejects_multiple_plus_4: "5M+T+B",
    }

    macro_rules! test_read_structure_from_str_invalid {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (input, expected) = $value;
                let actual = ReadStructure::from_str(input);
                assert!(actual.unwrap_err().to_string().ends_with(expected));
            }
        )*
        }
    }

    test_read_structure_from_str_invalid! {
        test_read_structure_from_str_invalid_0: ("9R", "[9R]"),
        test_read_structure_from_str_invalid_1: ("T", "[T]"),
        test_read_structure_from_str_invalid_2: ("23TT", "23T[T]"),
        test_read_structure_from_str_invalid_3: ("23T2", "23T[2]"),
        test_read_structure_from_str_invalid_4: ("23T2TT23T", "23T2T[T]23T"),
    }

    #[test]
    fn test_read_structure_collect_segments() {
        let rs = ReadStructure::from_str("10M9T8B7S3C10M9T8B7S2C").unwrap();
        let templates: String = rs.templates().map(|s| s.to_string()).collect();
        assert_eq!(templates, "9T9T");
        let sample_barcodes: String = rs.sample_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(sample_barcodes, "8B8B");
        let molecular_barcodes: String = rs.molecular_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(molecular_barcodes, "10M10M");
        let skips: String = rs.skips().map(|s| s.to_string()).collect();
        assert_eq!(skips, "7S7S");
        let cellular_barcodes: String = rs.cellular_barcodes().map(|s| s.to_string()).collect();
        assert_eq!(cellular_barcodes, "3C2C");
    }

    macro_rules! test_read_structure_length {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (input, expected) = $value;
                let actual = ReadStructure::from_str(input).unwrap().number_of_segments();
                assert_eq!(actual, expected);
            }
        )*
        }
    }

    test_read_structure_length! {
        test_read_structure_length_0: ("1T", 1),
        test_read_structure_length_1: ("1B", 1),
        test_read_structure_length_2: ("1M", 1),
        test_read_structure_length_3: ("1S", 1),
        test_read_structure_length_4: ("101T", 1),
        test_read_structure_length_5: ("5B101T", 2),
        test_read_structure_length_6: ("123456789T", 1),
        test_read_structure_length_7: ("10T10B10B10S10M", 5),
    }

    macro_rules! test_read_structure_index {
        ($($name:ident: $value:expr_2021,)*) => {
        $(
            #[test]
            fn $name() {
                let (string, index, exp_string) = $value;
                let read_structure = ReadStructure::from_str(string).unwrap();
                let read_segment = read_structure[index];
                assert_eq!(read_segment.to_string(), exp_string);
            }
        )*
        }
    }

    test_read_structure_index! {
        test_read_structure_index_0: ("1T", 0, "1T"),
        test_read_structure_index_1: ("1B", 0, "1B"),
        test_read_structure_index_2: ("1M", 0, "1M"),
        test_read_structure_index_3: ("1S", 0, "1S"),
        test_read_structure_index_4: ("101T", 0, "101T"),
        test_read_structure_index_5: ("5B101T", 0, "5B"),
        test_read_structure_index_6: ("5B101T", 1, "101T"),
        test_read_structure_index_7: ("123456789T", 0, "123456789T"),
        test_read_structure_index_8: ("10T10B10B10S10M", 0, "10T"),
        test_read_structure_index_9: ("10T10B10B10S10M", 1, "10B"),
        test_read_structure_index_10: ("10T10B10B10S10M", 2, "10B"),
        test_read_structure_index_11: ("10T10B10B10S10M", 3, "10S"),
        test_read_structure_index_12: ("10T10B10B10S10M", 4, "10M"),
        test_read_structure_index_32: ("10T10B10B10S10C10M", 4, "10C"),
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde() {
        let rs = ReadStructure::from_str("10T10B10B10S10M").unwrap();
        let rs_json = serde_json::to_string(&rs).unwrap();
        let rs2 = serde_json::from_str(&rs_json).unwrap();
        assert_eq!(rs, rs2);
    }

    // ---- non-terminal `+` acceptance (parsing + round-trip) ----

    #[test]
    fn test_accepts_middle_plus() {
        let rs = ReadStructure::from_str("8B+M10T").unwrap();
        assert_eq!(rs.to_string(), "8B+M10T");
        assert_eq!(rs.number_of_segments(), 3);
    }

    #[test]
    fn test_accepts_leading_plus() {
        let rs = ReadStructure::from_str("+B10T").unwrap();
        assert_eq!(rs.to_string(), "+B10T");
        assert_eq!(rs.number_of_segments(), 2);
    }

    #[test]
    fn test_accepts_middle_plus_between_fixed_runs() {
        let rs = ReadStructure::from_str("10T8B+M10T").unwrap();
        assert_eq!(rs.to_string(), "10T8B+M10T");
        assert_eq!(rs.number_of_segments(), 4);
    }

    // ---- has_fixed_length / fixed_length ----

    #[test]
    fn test_has_fixed_length_strict() {
        assert!(ReadStructure::from_str("10T8B").unwrap().has_fixed_length());
        assert!(!ReadStructure::from_str("10T+M").unwrap().has_fixed_length());
    }

    #[test]
    fn test_has_fixed_length_middle_plus() {
        assert!(!ReadStructure::from_str("8B+M10T").unwrap().has_fixed_length());
    }

    #[test]
    fn test_fixed_length_none_for_middle_plus() {
        assert!(ReadStructure::from_str("8B+M10T").unwrap().fixed_length().is_none());
    }

    // ---- extraction via ReadStructure::extract ----

    #[test]
    fn test_extract_fixed_length() {
        let rs = ReadStructure::from_str("10T8B").unwrap();
        let bases = b"AAAAAAAAAAGGGGGGGG";
        let quals = b"IIIIIIIIIIJJJJJJJJ";
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0.kind, SegmentType::Template);
        assert_eq!(out[0].1, b"AAAAAAAAAA");
        assert_eq!(out[0].2, b"IIIIIIIIII");
        assert_eq!(out[1].0.kind, SegmentType::SampleBarcode);
        assert_eq!(out[1].1, b"GGGGGGGG");
        assert_eq!(out[1].2, b"JJJJJJJJ");
    }

    #[test]
    fn test_extract_trailing_plus() {
        let rs = ReadStructure::from_str("10T+M").unwrap();
        let bases = b"AAAAAAAAAAGGGGGGGGGG";
        let quals = b"IIIIIIIIIIJJJJJJJJJJ";
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, b"AAAAAAAAAA");
        assert_eq!(out[1].1, b"GGGGGGGGGG");
        assert_eq!(out[1].2, b"JJJJJJJJJJ");
    }

    #[test]
    fn test_extract_leading_plus() {
        let rs = ReadStructure::from_str("+B10T").unwrap();
        let bases = b"BBBBBTTTTTTTTTT";
        let quals = b"!!!!!##########";
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0.kind, SegmentType::SampleBarcode);
        assert_eq!(out[0].1, b"BBBBB");
        assert_eq!(out[0].2, b"!!!!!");
        assert_eq!(out[1].0.kind, SegmentType::Template);
        assert_eq!(out[1].1, b"TTTTTTTTTT");
        assert_eq!(out[1].2, b"##########");
    }

    #[test]
    fn test_extract_middle_plus() {
        let rs = ReadStructure::from_str("8B+M10T").unwrap();
        let bases = b"BBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
        let quals = b"!!!!!!!!@@@@@@@@@@@@##########";
        assert_eq!(bases.len(), 30);
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].1, b"BBBBBBBB");
        assert_eq!(out[0].2, b"!!!!!!!!");
        assert_eq!(out[1].0.kind, SegmentType::MolecularBarcode);
        assert_eq!(out[1].1, b"UUUUUUUUUUUU");
        assert_eq!(out[1].2, b"@@@@@@@@@@@@");
        assert_eq!(out[2].1, b"TTTTTTTTTT");
        assert_eq!(out[2].2, b"##########");
    }

    #[test]
    fn test_extract_multiple_pre_plus_and_post_plus() {
        // Two pre-plus, one middle plus, one post-plus.
        let rs = ReadStructure::from_str("10T8B+M10T").unwrap();
        let bases = b"TTTTTTTTTTBBBBBBBBUUUUUUUUUUUUTTTTTTTTTT";
        let quals = b"IIIIIIIIII!!!!!!!!@@@@@@@@@@@@##########";
        assert_eq!(bases.len(), 40);
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].1, b"TTTTTTTTTT");
        assert_eq!(out[1].1, b"BBBBBBBB");
        assert_eq!(out[2].1, b"UUUUUUUUUUUU");
        assert_eq!(out[3].1, b"TTTTTTTTTT");
    }

    #[test]
    fn test_extract_include_skips_false_drops_skip() {
        let rs = ReadStructure::from_str("8S+M10T").unwrap();
        let bases = b"SSSSSSSSUUUUUUUUUUUUTTTTTTTTTT";
        let quals = b"????????@@@@@@@@@@@@##########";
        let out: Vec<_> = rs.extract(bases, quals, false).unwrap().collect();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0.kind, SegmentType::MolecularBarcode);
        assert_eq!(out[1].0.kind, SegmentType::Template);
    }

    #[test]
    fn test_extract_include_skips_true_keeps_skip() {
        let rs = ReadStructure::from_str("8S+M10T").unwrap();
        let bases = b"SSSSSSSSUUUUUUUUUUUUTTTTTTTTTT";
        let quals = b"????????@@@@@@@@@@@@##########";
        let out: Vec<_> = rs.extract(bases, quals, true).unwrap().collect();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].0.kind, SegmentType::Skip);
        assert_eq!(out[0].1, b"SSSSSSSS");
    }

    #[test]
    fn test_extract_errors_on_bases_quals_length_mismatch() {
        let rs = ReadStructure::from_str("10T").unwrap();
        let err = rs.extract(b"AAAAAAAAAA", b"III", true).unwrap_err();
        assert!(matches!(err, ReadStructureError::MismatchingBasesAndQualsLen { .. }));
    }

    #[test]
    fn test_extract_errors_when_read_too_short_for_fixed() {
        let rs = ReadStructure::from_str("10T8B").unwrap();
        let err = rs.extract(b"AAAA", b"IIII", true).unwrap_err();
        match err {
            ReadStructureError::ReadTooShort { read_len, required } => {
                assert_eq!(read_len, 4);
                assert_eq!(required, 18);
            }
            other => panic!("expected ReadTooShort, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_errors_when_read_exactly_fixed_len_but_plus_present() {
        // `+` requires at least one base, so read length must exceed fixed-length total.
        let rs = ReadStructure::from_str("8B+M10T").unwrap();
        let bases = vec![b'X'; 18]; // == length_of_fixed_segments
        let quals = vec![b'#'; 18];
        let err = rs.extract(&bases, &quals, true).unwrap_err();
        match err {
            ReadStructureError::ReadTooShort { read_len, required } => {
                assert_eq!(read_len, 18);
                assert_eq!(required, 19);
            }
            other => panic!("expected ReadTooShort, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_allows_read_exactly_fixed_len_when_no_plus() {
        let rs = ReadStructure::from_str("10T8B").unwrap();
        let bases = vec![b'X'; 18];
        let quals = vec![b'#'; 18];
        let out: Vec<_> = rs.extract(&bases, &quals, true).unwrap().collect();
        assert_eq!(out.len(), 2);
    }
}
