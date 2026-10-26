use std::collections::HashSet;

use super::{PALETTE_ID_MAX_BYTES, PALETTE_RECENT_MAX};

/// Validate a persisted palette identifier without normalization.
pub fn is_valid_palette_id(id: &str) -> bool {
    if id.is_empty() || id.len() > PALETTE_ID_MAX_BYTES {
        return false;
    }

    let mut segments = id.split(':');
    let Some(first) = segments.next() else {
        return false;
    };
    if !valid_first_segment(first) {
        return false;
    }

    let mut extra_segments = 0;
    for segment in segments {
        extra_segments += 1;
        if extra_segments > 2 || !valid_later_segment(segment) {
            return false;
        }
    }
    true
}

fn valid_first_segment(segment: &str) -> bool {
    valid_segment(segment, |byte| byte.is_ascii_lowercase())
}

fn valid_later_segment(segment: &str) -> bool {
    valid_segment(segment, |byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit()
    })
}

fn valid_segment(segment: &str, valid_first: impl Fn(u8) -> bool) -> bool {
    let bytes = segment.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 || !valid_first(bytes[0]) {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// Drop invalid identifiers and duplicates, retaining the first occurrence.
pub(super) fn sanitize_palette_recent(list: &mut Vec<String>) -> (usize, usize) {
    let mut sanitized = Vec::with_capacity(list.len().min(PALETTE_RECENT_MAX));
    let mut seen = HashSet::with_capacity(list.len());
    let mut dropped_invalid = 0;
    let mut truncated = 0;

    for id in list.drain(..) {
        if !is_valid_palette_id(&id) {
            dropped_invalid += 1;
        } else if seen.insert(id.clone()) {
            if sanitized.len() < PALETTE_RECENT_MAX {
                sanitized.push(id);
            } else {
                truncated += 1;
            }
        }
    }

    *list = sanitized;
    (dropped_invalid, truncated)
}

/// Move a valid identifier to the front, retaining at most 20 entries.
pub(super) fn push_recent(list: &mut Vec<String>, id: &str) -> bool {
    if !is_valid_palette_id(id) {
        return false;
    }

    let recent = list
        .iter()
        .position(|existing| existing == id)
        .map(|index| list.remove(index))
        .unwrap_or_else(|| id.to_owned());
    list.retain(|existing| existing != id);
    list.insert(0, recent);
    list.truncate(PALETTE_RECENT_MAX);
    true
}
