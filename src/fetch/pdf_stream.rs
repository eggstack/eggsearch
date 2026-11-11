//! Bounded pre-parse scan of PDF stream expansion.
//!
//! `lopdf` inflates Flate and LZW streams without an output limit, and
//! `decode_xref_stream` allocates a row buffer from the declared `/W` widths
//! before it reads an entry. This module walks the raw buffer before any parse,
//! measures each declared stream against a byte budget, and refuses the
//! document at the first breach.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::{DeflateDecoder, ZlibDecoder};

use super::types::FetchError;

const STREAM_KEYWORD: &[u8] = b"stream";
const ENDSTREAM_KEYWORD: &[u8] = b"endstream";
const FLATE_FILTER: &[u8] = b"FlateDecode";
const ASCII85_FILTER: &[u8] = b"ASCII85Decode";
const LZW_FILTER: &[u8] = b"LZWDecode";

/// Largest declared xref field width accepted from `/W`.
const MAX_XREF_FIELD_WIDTH: i64 = 8;

/// Bytes requested per inflate read. A breach is reported within one block of
/// the stream's remaining allowance.
const INFLATE_BLOCK: usize = 8192;

const DICTIONARY_DELIMITERS: &[u8] = b"()<>[]{}/%";

#[derive(Debug, PartialEq, Eq)]
enum Ascii85Outcome {
    Decoded(Vec<u8>, bool),
    Invalid,
}

#[derive(Default)]
struct StreamDict<'a> {
    length: Option<usize>,
    length_is_indirect: bool,
    filters: Vec<&'a [u8]>,
    is_xref: bool,
    xref_widths: Option<Vec<i64>>,
    predictor_row: Option<usize>,
}

/// Measure the decompressed size of every stream in `bytes` and refuse the
/// document when the total would exceed `budget`.
///
/// Only materialised expansion is counted. A stream with no `/Filter` is used
/// verbatim and bounded by the response cap, so it contributes nothing. A
/// filter `lopdf` cannot decode stops the chain, and the stages decoded before
/// it are still counted because `lopdf` inflates them before returning its
/// `Unimplemented` error. `FlateDecode` is inflated through a reader capped at
/// the remaining allowance; `ASCII85Decode` is decoded under the same cap; an
/// `LZWDecode` stream cannot be measured and is refused.
pub fn scan_stream_budget(bytes: &[u8], budget: usize) -> Result<usize, FetchError> {
    let mut total = 0usize;
    let mut search_from = 0usize;

    while let Some(keyword) = next_stream_keyword(bytes, search_from) {
        let info = match enclosing_dictionary(bytes, search_from, keyword) {
            Some((start, end)) => parse_stream_dict(&bytes[start..end]),
            None => StreamDict::default(),
        };
        let content_start = content_offset(bytes, keyword);
        let (content, next) = stream_content(bytes, content_start, info.length);
        let remaining = budget.saturating_sub(total);

        if info.is_xref || info.xref_widths.is_some() {
            check_xref_geometry(&info, remaining, total, budget)?;
        }
        if info.length_is_indirect && !info.filters.is_empty() {
            return Err(bound_exceeded(budget.saturating_add(1), budget));
        }
        if info.filters.contains(&FLATE_FILTER) {
            if let Some(row) = info.predictor_row {
                if row > remaining {
                    return Err(bound_exceeded(total.saturating_add(row), budget));
                }
            }
        }

        let expanded = measure_stream(content, &info, remaining, total, budget)?;
        total = total.saturating_add(expanded);
        search_from = next.max(keyword + STREAM_KEYWORD.len());
    }

    Ok(total)
}

fn bound_exceeded(measured: usize, budget: usize) -> FetchError {
    FetchError::PdfDecompressionBoundExceeded { measured, budget }
}

fn check_xref_geometry(
    info: &StreamDict<'_>,
    remaining: usize,
    total: usize,
    budget: usize,
) -> Result<(), FetchError> {
    let widths = match info.xref_widths {
        Some(ref widths) if widths.len() >= 3 => widths,
        _ => return Ok(()),
    };
    if widths.iter().any(|width| *width < 0) {
        return Ok(());
    }
    if widths.iter().any(|width| *width > MAX_XREF_FIELD_WIDTH) {
        return Err(bound_exceeded(budget.saturating_add(1), budget));
    }
    let row = widths.iter().fold(0usize, |acc, width| {
        acc.saturating_add(usize::try_from(*width).unwrap_or(usize::MAX))
    });
    if row > remaining {
        return Err(bound_exceeded(total.saturating_add(row), budget));
    }
    Ok(())
}

fn measure_stream(
    content: &[u8],
    info: &StreamDict<'_>,
    remaining: usize,
    total: usize,
    budget: usize,
) -> Result<usize, FetchError> {
    if info.filters.is_empty() {
        return Ok(0);
    }

    let mut data: Cow<'_, [u8]> = Cow::Borrowed(content);
    let mut peak = 0usize;

    for filter in &info.filters {
        let next = match *filter {
            FLATE_FILTER => {
                let (decoded, capped) = inflate_bounded(data.as_ref(), remaining);
                if capped {
                    return Err(bound_exceeded(total.saturating_add(decoded.len()), budget));
                }
                Cow::Owned(decoded)
            }
            ASCII85_FILTER => match decode_ascii85(data.as_ref(), remaining) {
                Ascii85Outcome::Decoded(decoded, capped) => {
                    if capped {
                        return Err(bound_exceeded(total.saturating_add(decoded.len()), budget));
                    }
                    Cow::Owned(decoded)
                }
                Ascii85Outcome::Invalid => return Ok(peak),
            },
            LZW_FILTER => return Err(bound_exceeded(budget.saturating_add(1), budget)),
            _ => return Ok(peak),
        };
        data = next;
        if data.len() > remaining {
            return Err(bound_exceeded(total.saturating_add(data.len()), budget));
        }
        peak = peak.max(data.len());
    }

    Ok(peak)
}

fn inflate_bounded(input: &[u8], limit: usize) -> (Vec<u8>, bool) {
    if input.is_empty() {
        return (Vec::new(), false);
    }

    let (decoded, capped) = read_capped(ZlibDecoder::new(input), limit);
    if !capped && decoded.is_empty() && input.len() > 2 {
        return read_capped(DeflateDecoder::new(&input[2..]), limit);
    }
    (decoded, capped)
}

fn read_capped<R: Read>(mut reader: R, limit: usize) -> (Vec<u8>, bool) {
    let mut decoded = Vec::new();
    let mut block = [0u8; INFLATE_BLOCK];
    loop {
        if decoded.len() > limit {
            return (decoded, true);
        }
        let want = limit
            .saturating_sub(decoded.len())
            .saturating_add(1)
            .min(INFLATE_BLOCK);
        match reader.read(&mut block[..want]) {
            Ok(0) | Err(_) => return (decoded, false),
            Ok(read) => decoded.extend_from_slice(&block[..read]),
        }
    }
}

fn decode_ascii85(input: &[u8], limit: usize) -> Ascii85Outcome {
    let body = if input.len() >= 2 && &input[input.len() - 2..] == b"~>" {
        &input[..input.len() - 2]
    } else {
        input
    };

    let mut decoded = Vec::new();
    let mut buffer: u32 = 0;
    let mut count = 0usize;

    for &byte in body {
        if byte == b'z' {
            if count != 0 {
                return Ascii85Outcome::Invalid;
            }
            decoded.extend_from_slice(&[0, 0, 0, 0]);
            if decoded.len() > limit {
                return Ascii85Outcome::Decoded(decoded, true);
            }
            continue;
        }
        if byte.is_ascii_whitespace() {
            continue;
        }
        if !(b'!'..=b'u').contains(&byte) {
            break;
        }
        buffer = match buffer
            .checked_mul(85)
            .and_then(|value| value.checked_add(u32::from(byte - b'!')))
        {
            Some(value) => value,
            None => return Ascii85Outcome::Invalid,
        };
        count += 1;
        if count == 5 {
            decoded.extend_from_slice(&buffer.to_be_bytes());
            buffer = 0;
            count = 0;
            if decoded.len() > limit {
                return Ascii85Outcome::Decoded(decoded, true);
            }
        }
    }

    if count > 0 {
        for _ in count..5 {
            buffer = match buffer
                .checked_mul(85)
                .and_then(|value| value.checked_add(84))
            {
                Some(value) => value,
                None => return Ascii85Outcome::Invalid,
            };
        }
        decoded.extend_from_slice(&buffer.to_be_bytes()[..count - 1]);
    }

    let capped = decoded.len() > limit;
    Ascii85Outcome::Decoded(decoded, capped)
}

fn next_stream_keyword(bytes: &[u8], from: usize) -> Option<usize> {
    let mut from = from;
    while let Some(position) = find_subslice(bytes, STREAM_KEYWORD, from) {
        let is_endstream = position >= 3 && &bytes[position - 3..position] == b"end";
        let before_ok = match position.checked_sub(1) {
            None => true,
            Some(index) => is_whitespace(bytes[index]) || bytes[index] == b'>',
        };
        let after = position + STREAM_KEYWORD.len();
        let after_ok = matches!(bytes.get(after), Some(b'\r') | Some(b'\n'));
        if !is_endstream && before_ok && after_ok {
            return Some(position);
        }
        from = position + STREAM_KEYWORD.len();
    }
    None
}

fn content_offset(bytes: &[u8], keyword: usize) -> usize {
    let mut offset = keyword + STREAM_KEYWORD.len();
    if bytes.get(offset) == Some(&b'\r') {
        offset += 1;
    }
    if bytes.get(offset) == Some(&b'\n') {
        offset += 1;
    }
    offset
}

fn stream_content(bytes: &[u8], start: usize, declared: Option<usize>) -> (&[u8], usize) {
    if let Some(length) = declared {
        if let Some(end) = start.checked_add(length) {
            if end <= bytes.len() {
                let mut cursor = skip_ws(bytes, end);
                if bytes[cursor..].starts_with(ENDSTREAM_KEYWORD) {
                    cursor += ENDSTREAM_KEYWORD.len();
                    return (&bytes[start..end], cursor);
                }
            }
        }
    }

    match find_subslice(bytes, ENDSTREAM_KEYWORD, start) {
        Some(end) => (&bytes[start..end], end + ENDSTREAM_KEYWORD.len()),
        None => (&bytes[start..], bytes.len()),
    }
}

fn enclosing_dictionary(bytes: &[u8], from: usize, keyword: usize) -> Option<(usize, usize)> {
    let mut cursor = from;
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut last: Option<(usize, usize)> = None;

    while cursor + 1 < keyword {
        if bytes[cursor] == b'%' {
            cursor = skip_comment(bytes, cursor);
            continue;
        }
        if bytes[cursor] == b'(' {
            let end = skip_literal_string(bytes, cursor);
            cursor = if end > keyword { cursor + 1 } else { end };
            continue;
        }
        if bytes[cursor] == b'<' && bytes[cursor + 1] == b'<' {
            if depth == 0 {
                start = cursor;
            }
            depth += 1;
            cursor += 2;
            continue;
        }
        if bytes[cursor] == b'>' && bytes[cursor + 1] == b'>' && depth > 0 {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                last = Some((start, cursor));
            }
            continue;
        }
        if bytes[cursor] == b'<' {
            let end = skip_hex_string(bytes, cursor);
            cursor = if end > keyword { cursor + 1 } else { end };
            continue;
        }
        cursor += 1;
    }

    let (start, end) = last?;
    let mut gap = end;
    while gap < keyword {
        if is_whitespace(bytes[gap]) {
            gap += 1;
        } else if bytes[gap] == b'%' {
            gap = skip_comment(bytes, gap);
        } else {
            return None;
        }
    }
    Some((start, end))
}

fn parse_stream_dict(dict: &[u8]) -> StreamDict<'_> {
    let mut info = StreamDict::default();
    let mut depth = 0usize;
    let mut cursor = 0usize;

    while cursor < dict.len() {
        match dict[cursor] {
            b'%' => cursor = skip_comment(dict, cursor),
            b'<' if dict.get(cursor + 1) == Some(&b'<') => {
                depth += 1;
                cursor += 2;
            }
            b'>' if dict.get(cursor + 1) == Some(&b'>') => {
                depth = depth.saturating_sub(1);
                cursor += 2;
            }
            b'<' => cursor = skip_hex_string(dict, cursor),
            b'(' => cursor = skip_literal_string(dict, cursor),
            b'/' if depth == 1 => {
                let name_end = name_end(dict, cursor + 1);
                let name = &dict[cursor + 1..name_end];
                let value_start = skip_ws(dict, name_end);
                let value_end = value_end(dict, value_start);
                let value = &dict[value_start..value_end];
                match name {
                    b"Length" => {
                        if info.length.is_none() && !info.length_is_indirect {
                            if is_indirect_reference(&dict[value_end..]) {
                                info.length_is_indirect = true;
                            } else {
                                info.length = parse_integer(value)
                                    .and_then(|value| usize::try_from(value).ok());
                            }
                        }
                    }
                    b"Filter" => info.filters = parse_filter_names(value),
                    b"Type" => info.is_xref = value == b"/XRef",
                    b"W" => info.xref_widths = parse_integer_array(value),
                    b"DecodeParms" => info.predictor_row = predictor_row(value),
                    _ => {}
                }
                cursor = value_end.max(name_end).max(cursor + 1);
            }
            _ => cursor += 1,
        }
    }

    info
}

fn predictor_row(value: &[u8]) -> Option<usize> {
    if !value.starts_with(b"<<") {
        return None;
    }

    let mut predictor = 1i64;
    let mut columns = 1i64;
    let mut colors = 1i64;
    let mut bits = 8i64;
    let mut cursor = 2usize;

    while cursor < value.len() {
        if value[cursor] == b'/' {
            let end = name_end(value, cursor + 1);
            let name = &value[cursor + 1..end];
            let start = skip_ws(value, end);
            let stop = value_end(value, start);
            let token = &value[start..stop];
            match name {
                b"Predictor" => predictor = parse_integer(token).unwrap_or(1),
                b"Columns" => columns = parse_integer(token).unwrap_or(1),
                b"Colors" => colors = parse_integer(token).unwrap_or(1),
                b"BitsPerComponent" => bits = parse_integer(token).unwrap_or(8),
                _ => {}
            }
            cursor = stop.max(end).max(cursor + 1);
        } else {
            cursor += 1;
        }
    }

    if !(10..=15).contains(&predictor) {
        return None;
    }

    let colors = usize::try_from(colors.max(1)).unwrap_or(usize::MAX);
    let bits = usize::try_from(bits.max(8)).unwrap_or(usize::MAX);
    let columns = usize::try_from(columns.max(1)).unwrap_or(usize::MAX);
    Some(
        colors
            .saturating_mul(bits)
            .saturating_div(8)
            .saturating_mul(columns),
    )
}

fn parse_filter_names(value: &[u8]) -> Vec<&[u8]> {
    if value.first() == Some(&b'/') {
        return vec![&value[1..name_end(value, 1)]];
    }
    if value.first() == Some(&b'[') {
        let mut names = Vec::new();
        let mut cursor = 1usize;
        while cursor < value.len() && value[cursor] != b']' {
            if value[cursor] == b'/' {
                let end = name_end(value, cursor + 1);
                names.push(&value[cursor + 1..end]);
                cursor = end.max(cursor + 1);
            } else {
                cursor += 1;
            }
        }
        return names;
    }
    Vec::new()
}

fn parse_integer_array(value: &[u8]) -> Option<Vec<i64>> {
    if value.first() != Some(&b'[') {
        return None;
    }
    let mut integers = Vec::new();
    let mut cursor = 1usize;
    while cursor < value.len() && value[cursor] != b']' {
        if is_whitespace(value[cursor]) {
            cursor += 1;
            continue;
        }
        let end = token_end(value, cursor);
        if end == cursor {
            cursor += 1;
            continue;
        }
        integers.push(parse_integer(&value[cursor..end])?);
        cursor = end;
    }
    Some(integers)
}

fn parse_integer(token: &[u8]) -> Option<i64> {
    std::str::from_utf8(token).ok()?.parse().ok()
}

fn is_indirect_reference(rest: &[u8]) -> bool {
    let mut cursor = skip_ws(rest, 0);
    let number_end = token_end(rest, cursor);
    if number_end == cursor || parse_integer(&rest[cursor..number_end]).is_none() {
        return false;
    }
    cursor = skip_ws(rest, number_end);
    if rest.get(cursor) != Some(&b'R') {
        return false;
    }
    match rest.get(cursor + 1) {
        None => true,
        Some(&byte) => is_whitespace(byte) || is_delimiter(byte),
    }
}

fn value_end(bytes: &[u8], start: usize) -> usize {
    match bytes.get(start) {
        None => start,
        Some(b'<') if bytes.get(start + 1) == Some(&b'<') => skip_nested_dict(bytes, start),
        Some(b'[') => skip_array(bytes, start),
        Some(b'(') => skip_literal_string(bytes, start),
        Some(b'/') => token_end(bytes, start + 1),
        Some(b'<') => skip_hex_string(bytes, start),
        Some(_) => token_end(bytes, start),
    }
}

fn token_end(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() && !is_whitespace(bytes[cursor]) && !is_delimiter(bytes[cursor]) {
        cursor += 1;
    }
    cursor
}

fn name_end(bytes: &[u8], start: usize) -> usize {
    token_end(bytes, start)
}

fn skip_comment(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor] != b'\n' && bytes[cursor] != b'\r' {
        cursor += 1;
    }
    cursor
}

fn skip_ws(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() {
        if is_whitespace(bytes[cursor]) {
            cursor += 1;
        } else if bytes[cursor] == b'%' {
            cursor = skip_comment(bytes, cursor);
        } else {
            break;
        }
    }
    cursor
}

fn skip_literal_string(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start + 1;
    let mut depth = 1usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            b'(' => {
                depth += 1;
                cursor += 1;
            }
            b')' => {
                depth -= 1;
                cursor += 1;
                if depth == 0 {
                    return cursor;
                }
            }
            _ => cursor += 1,
        }
    }
    bytes.len()
}

fn skip_hex_string(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'>' {
            return cursor + 1;
        }
        cursor += 1;
    }
    bytes.len()
}

fn skip_nested_dict(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    let mut depth = 0usize;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'<' && bytes[cursor + 1] == b'<' {
            depth += 1;
            cursor += 2;
            continue;
        }
        if bytes[cursor] == b'>' && bytes[cursor + 1] == b'>' {
            depth = depth.saturating_sub(1);
            cursor += 2;
            if depth == 0 {
                return cursor;
            }
            continue;
        }
        if bytes[cursor] == b'(' {
            cursor = skip_literal_string(bytes, cursor);
            continue;
        }
        cursor += 1;
    }
    bytes.len()
}

fn skip_array(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start + 1;
    let mut depth = 1usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'[' => {
                depth += 1;
                cursor += 1;
            }
            b']' => {
                depth -= 1;
                cursor += 1;
                if depth == 0 {
                    return cursor;
                }
            }
            b'(' => cursor = skip_literal_string(bytes, cursor),
            b'<' if bytes.get(cursor + 1) == Some(&b'<') => {
                cursor = skip_nested_dict(bytes, cursor)
            }
            b'<' => cursor = skip_hex_string(bytes, cursor),
            _ => cursor += 1,
        }
    }
    bytes.len()
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn is_delimiter(byte: u8) -> bool {
    DICTIONARY_DELIMITERS.contains(&byte)
}

fn find_subslice(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() || haystack.len() - from < needle.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    const SMALL_BUDGET: usize = 4096;

    fn deflate(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(data).expect("deflate writes");
        encoder.finish().expect("deflate finishes")
    }

    fn ascii85(data: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        for chunk in data.chunks(4) {
            let mut buffer = [0u8; 4];
            buffer[..chunk.len()].copy_from_slice(chunk);
            let mut value = u32::from_be_bytes(buffer);
            if chunk.len() == 4 {
                let mut digits = [0u8; 5];
                for digit in digits.iter_mut().rev() {
                    *digit = b'!' + (value % 85) as u8;
                    value /= 85;
                }
                encoded.extend_from_slice(&digits);
            } else {
                let mut digits = [0u8; 5];
                for digit in digits.iter_mut().rev() {
                    *digit = b'!' + (value % 85) as u8;
                    value /= 85;
                }
                encoded.extend_from_slice(&digits[..chunk.len() + 1]);
            }
        }
        encoded.extend_from_slice(b"~>");
        encoded
    }

    fn stream(document_dict: &str, content: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(format!("<< {document_dict} >>\nstream\n").as_bytes());
        out.extend_from_slice(content);
        out.extend_from_slice(b"\nendstream\n");
        out
    }

    fn flate_stream(output_size: usize) -> Vec<u8> {
        deflate(&vec![b'a'; output_size])
    }

    #[test]
    fn uncompressed_stream_contributes_nothing() {
        let bytes = stream("/Type /Foo", b"plain bytes");
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 0);
    }

    #[test]
    fn flate_stream_is_measured() {
        let payload = b"hello hello hello hello".repeat(10);
        let bytes = stream("/Filter /FlateDecode", &deflate(&payload));
        assert_eq!(
            scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(),
            payload.len()
        );
    }

    #[test]
    fn flate_breach_stops_within_one_block() {
        let bytes = stream("/Filter /FlateDecode", &flate_stream(SMALL_BUDGET * 8));
        let error = scan_stream_budget(&bytes, SMALL_BUDGET).unwrap_err();
        match error {
            FetchError::PdfDecompressionBoundExceeded { measured, budget } => {
                assert_eq!(budget, SMALL_BUDGET);
                assert!(
                    measured > budget,
                    "measured {measured} must exceed {budget}"
                );
                assert!(
                    measured <= budget + INFLATE_BLOCK,
                    "measured {measured} must stay within one block of {budget}"
                );
            }
            other => panic!("expected a bound breach, got {other:?}"),
        }
    }

    #[test]
    fn remaining_allowance_is_shared_across_streams() {
        let payload = vec![b'b'; SMALL_BUDGET * 3 / 5];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&stream("/Filter /FlateDecode", &deflate(&payload)));
        bytes.extend_from_slice(&stream("/Filter /FlateDecode", &deflate(&payload)));
        assert!(scan_stream_budget(&bytes, SMALL_BUDGET * 2).is_ok());
        assert!(scan_stream_budget(&bytes, SMALL_BUDGET).is_err());
    }

    #[test]
    fn endstream_fallback_survives_a_wrong_length() {
        let payload = b"fallback payload".repeat(20);
        let compressed = deflate(&payload);
        let mut bytes = b"<< /Length 5 /Filter /FlateDecode >>\nstream\n".to_vec();
        bytes.extend_from_slice(&compressed);
        bytes.extend_from_slice(b"\nendstream\n");
        assert_eq!(
            scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(),
            payload.len()
        );
    }

    #[test]
    fn indirect_length_with_a_filter_is_refused() {
        let payload = b"indirect length".repeat(10);
        let compressed = deflate(&payload);
        let mut bytes = b"<< /Length 9 0 R /Filter /FlateDecode >>\nstream\n".to_vec();
        bytes.extend_from_slice(&compressed);
        bytes.extend_from_slice(b"\nendstream\n");
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn indirect_length_without_a_filter_is_accepted() {
        let bytes = b"<< /Length 9 0 R >>\nstream\nplain bytes\nendstream\n".to_vec();
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 0);
    }

    #[test]
    fn comment_between_the_dictionary_and_stream_does_not_hide_the_filter() {
        let payload = vec![b'c'; SMALL_BUDGET * 8];
        let compressed = deflate(&payload);
        let mut bytes = format!(
            "<< /Filter /FlateDecode /Length {} >> % comment\nstream\n",
            compressed.len()
        )
        .into_bytes();
        bytes.extend_from_slice(&compressed);
        bytes.extend_from_slice(b"\nendstream\n");
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn ascii85_then_flate_chain_is_measured() {
        let payload = b"chained filter payload payload payload".repeat(8);
        let compressed = deflate(&payload);
        let encoded = ascii85(&compressed);
        let bytes = stream("/Filter [/ASCII85Decode /FlateDecode]", &encoded);
        assert_eq!(
            scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(),
            payload.len()
        );
    }

    #[test]
    fn chained_filter_breach_is_refused() {
        let bytes = stream(
            "/Filter [/ASCII85Decode /FlateDecode]",
            &ascii85(&flate_stream(SMALL_BUDGET * 8)),
        );
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn ascii85_shorthand_is_measured_under_the_cap() {
        let mut encoded = vec![b'z'; SMALL_BUDGET];
        encoded.extend_from_slice(b"~>");
        let bytes = stream("/Filter /ASCII85Decode", &encoded);
        let error = scan_stream_budget(&bytes, SMALL_BUDGET).unwrap_err();
        match error {
            FetchError::PdfDecompressionBoundExceeded { measured, budget } => {
                assert_eq!(budget, SMALL_BUDGET);
                assert!(measured > budget && measured <= budget + 4);
            }
            other => panic!("expected a bound breach, got {other:?}"),
        }
    }

    #[test]
    fn lzw_stream_is_refused_rather_than_inflated_unmeasured() {
        let bytes = stream("/Filter /LZWDecode", b"not really lzw");
        match scan_stream_budget(&bytes, SMALL_BUDGET) {
            Err(FetchError::PdfDecompressionBoundExceeded { measured, budget }) => {
                assert_eq!(budget, SMALL_BUDGET);
                assert!(measured > budget);
            }
            other => panic!("expected an LZW refusal, got {other:?}"),
        }
    }

    #[test]
    fn flate_before_an_unknown_filter_is_still_measured() {
        let bytes = stream(
            "/Filter [/FlateDecode /DCTDecode]",
            &flate_stream(SMALL_BUDGET * 8),
        );
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn unknown_filter_contributes_nothing() {
        let bytes = stream("/Filter /DCTDecode", &vec![0u8; SMALL_BUDGET * 4]);
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 0);
    }

    #[test]
    fn xref_field_width_above_eight_is_refused() {
        let bytes = stream("/Type /XRef /W [1 9 2] /Size 1 /Length 0", b"");
        match scan_stream_budget(&bytes, SMALL_BUDGET) {
            Err(FetchError::PdfDecompressionBoundExceeded { measured, budget }) => {
                assert_eq!(budget, SMALL_BUDGET);
                assert!(measured > budget);
            }
            other => panic!("expected a width refusal, got {other:?}"),
        }
    }

    #[test]
    fn xref_geometry_without_a_type_entry_is_still_refused() {
        let bytes = stream("/W [1 9 2] /Size 1 /Length 0", b"");
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn xref_row_width_above_remaining_is_refused() {
        let bytes = stream("/Type /XRef /W [1 8 8] /Size 1 /Length 0", b"");
        let error = scan_stream_budget(&bytes, 8).unwrap_err();
        assert!(matches!(
            error,
            FetchError::PdfDecompressionBoundExceeded {
                measured: 17,
                budget: 8
            }
        ));
    }

    #[test]
    fn flat_xref_geometry_is_accepted() {
        let bytes = stream("/Type /XRef /W [1 2 1] /Size 1 /Length 0", b"");
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 0);
    }

    #[test]
    fn oversized_predictor_row_is_refused() {
        let bytes = stream(
            "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 500000000 /Colors 1 /BitsPerComponent 8 >>",
            &deflate(b"tiny"),
        );
        assert!(matches!(
            scan_stream_budget(&bytes, SMALL_BUDGET),
            Err(FetchError::PdfDecompressionBoundExceeded { .. })
        ));
    }

    #[test]
    fn ordinary_predictor_row_is_accepted() {
        let bytes = stream(
            "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 4 /Colors 1 /BitsPerComponent 8 >>",
            &deflate(b"tiny"),
        );
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 4);
    }

    #[test]
    fn dictionary_strings_cannot_hide_the_filter() {
        let payload = b"string aware dictionary payload".repeat(10);
        let bytes = stream(
            "/Note (<< not a dictionary) /Also (>>) /Filter /FlateDecode",
            &deflate(&payload),
        );
        assert_eq!(
            scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(),
            payload.len()
        );
    }

    #[test]
    fn an_unterminated_string_cannot_blind_the_dictionary_scan() {
        let payload = b"defensive dictionary payload".repeat(10);
        let mut bytes = b"% stray ( comment\n".to_vec();
        bytes.extend_from_slice(&stream("/Filter /FlateDecode", &deflate(&payload)));
        assert_eq!(
            scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(),
            payload.len()
        );
    }

    #[test]
    fn endstream_keyword_is_not_a_stream_start() {
        let bytes = b"endstream\n".to_vec();
        assert_eq!(scan_stream_budget(&bytes, SMALL_BUDGET).unwrap(), 0);
    }

    #[test]
    fn empty_input_is_accepted() {
        assert_eq!(scan_stream_budget(b"", SMALL_BUDGET).unwrap(), 0);
    }
}
