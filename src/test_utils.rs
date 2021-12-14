use crate::{Event, EventKind, FieldValue, Span};
use itertools::Itertools;

use self::ser::SpanItem;

mod ser {
    pub use crate::subscriber::serialize::*;
    pub use FieldValue::*;
}

pub fn events(count: usize) -> Vec<Event> {
    todo!()
}

pub fn float_eq(a: f64, b: f64) -> bool {
    // Safety: f64 and u64 have the same size and alignment, and every 64-bit-pattern is
    // valid for u64.
    unsafe {
        let a: u64 = std::mem::transmute(a);
        let b: u64 = std::mem::transmute(b);
        a == b
    }
}

pub fn eq_field_values(a: &FieldValue, b: &ser::FieldValue) -> bool {
    use crate::FieldValue::*;

    match (a, b) {
        (Int(a), ser::Int(b)) => a == b,
        (Bool(a), ser::Bool(b)) => a == b,
        (Float(a), ser::Float(b)) => float_eq(*a, *b),
        (Str(a), ser::Str(b)) => a == b,
        _ => false,
    }
}

pub fn eq_kind(a: &EventKind, b: &ser::EventKind) -> bool {
    match (a, b) {
        (EventKind::Event(a_fields), ser::EventKind::Event(b_fields)) => {
            if a_fields.len() != b_fields.len() {
                return false;
            }
            for (name, val) in b_fields {
                match a_fields.get(*name) {
                    Some(v) if eq_field_values(v, val) => continue,
                    _ => return false,
                }
            }
            return true;
        }
        (EventKind::SpanCreate, ser::EventKind::SpanCreate) => true,
        (EventKind::SpanEnter, ser::EventKind::SpanEnter) => true,
        (EventKind::SpanExit, ser::EventKind::SpanExit) => true,
        (EventKind::SpanClose(a), ser::EventKind::SpanClose(b)) => a == b,
        _ => false,
    }
}

pub fn eq_span(a: &Span, b: &[ser::SpanItem]) -> bool {
    match &b[0] {
        ser::SpanItem::Field { .. } => panic!("first element should be SpanItem::Start"),
        ser::SpanItem::Start { span_name, id } => {
            if &a.id != id || a.name != *span_name {
                return false;
            }
            let fields = &b[1..];
            if a.fields.len() != fields.len() {
                return false;
            }
            for f in fields {
                match f {
                    ser::SpanItem::Field { name, val } => match a.fields.get(*name) {
                        Some(v) if eq_field_values(v, val) => continue,
                        _ => return false,
                    },
                    ser::SpanItem::Start { .. } => panic!("b contains multiple spans"),
                }
            }
        }
    }

    true
}

pub fn eq_spans(a: &[Span], b: &ser::Spans) -> bool {
    let b = b.as_items();
    let start_inds = b
        .iter()
        .enumerate()
        .filter_map(|(k, s)| match s {
            SpanItem::Field { .. } => None,
            SpanItem::Start { .. } => Some(k),
        })
        .chain(std::iter::once(b.len()));

    let mut span_idx = 0usize;

    for (start, end) in start_inds.tuple_windows() {
        if span_idx >= a.len() || !eq_span(&a[span_idx], &b[start..end]) {
            return false;
        }
        span_idx += 1
    }

    span_idx == a.len()
}

pub fn eq_event_ser_event(a: &Event, b: &ser::Event) -> bool {
    let Event {
        kind,
        level,
        target,
        spans,
        thread_id,
        thread_name,
        src_file,
        src_line,
        time,
    } = a;

    eq_kind(kind, &b.kind)
        && level == &b.level
        && target == &b.target
        && time == &b.time
        && thread_id == &b.thread_id
        && thread_name.as_ref().map(String::as_str) == b.thread_name
        && src_line == &b.src_line
        && src_file.as_ref().map(String::as_str) == b.src_file
        && eq_spans(spans, &b.spans)
}

pub fn eq_event(a: &Event, b: &Event) -> bool {
    let Event {
        kind,
        level,
        target,
        spans,
        thread_id,
        thread_name,
        src_file,
        src_line,
        time,
    } = a;

    if !(kind == &b.kind
        && level == &b.level
        && target == &b.target
        && time == &b.time
        && thread_id == &b.thread_id
        && thread_name == &b.thread_name
        && src_line == &b.src_line
        && src_file == &b.src_file
        && spans.len() == b.spans.len()
    ) {
        return false;
    }

    for (a, b) in spans.iter().zip(&b.spans) {
        if !(
            &a.name == &b.name
            && a.id == b.id
            && a.fields == b.fields
        ) {
            return false
        }
    }
    true    
}
