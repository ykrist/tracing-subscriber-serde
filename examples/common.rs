pub fn creates_spans_and_events() {
    use tracing::*;

    let _outer = warn_span!("outer", x = 6).entered();
    for i in 0..3 {
        let _a = error_span!("a", i, p = "egg").entered();
        error!(cat = true, bacon = 4, foo = "mao", "hello");
        let _b = debug_span!("check_for_egg", i).entered();
        if i % 2 == 0 {
            info!("egg");
            error!(eggy = "no")
        } else {
            trace!(foo = 42.0, "no\negg");
            debug!(a = 4, b = 1.4);
        }
    }
}
