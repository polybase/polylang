use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[macro_export]
macro_rules! maybe_start {
    ($span:expr) => {
        let _span;
        {
            let span: Option<_> = $span;
            if let Some(span) = span {
                _span = $crate::span::start(span);
            }
        }
    };
}

static CURRENT_SPAN: Mutex<Option<Span>> = Mutex::new(None);

pub struct SpanGuard(Option<Span>);

#[must_use]
pub fn start(span: impl Into<Span>) -> SpanGuard {
    let old = std::mem::replace(&mut *CURRENT_SPAN.lock(), Some(span.into()));
    SpanGuard(old)
}

pub(crate) fn get() -> Option<Span> {
    *CURRENT_SPAN.lock()
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        *CURRENT_SPAN.lock() = self.0;
    }
}
