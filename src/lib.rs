#![allow(dead_code)]
use time::{macros::offset, OffsetDateTime};

pub mod algrithm;
pub mod backtest;
pub mod binance_futures;
pub mod market;

pub fn init_log(file_name: &str) -> tracing_appender::non_blocking::WorkerGuard {
    let file_name = file_name.to_owned() + ".log";
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::time::FormatTime;
    use tracing_subscriber::FmtSubscriber;

    struct Timer;
    impl FormatTime for Timer {
        fn format_time(
            &self,
            w: &mut tracing_subscriber::fmt::format::Writer<'_>,
        ) -> std::fmt::Result {
            let now = local_now();
            let format =
                time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap();
            write!(w, "{}", now.format(&format).unwrap())
        }
    }
    let file_appender = tracing_appender::rolling::never("./logs", file_name);

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(non_blocking)
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_timer(Timer)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    _guard
}

pub fn local_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(offset!(+8))
}

pub fn stdout_logger() {
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::time::FormatTime;
    use tracing_subscriber::FmtSubscriber;
    struct Timer;
    impl FormatTime for Timer {
        fn format_time(
            &self,
            w: &mut tracing_subscriber::fmt::format::Writer<'_>,
        ) -> std::fmt::Result {
            let now = local_now();
            let format =
                time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
                    .unwrap();
            write!(w, "{}", now.format(&format).unwrap())
        }
    }
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_span_events(FmtSpan::CLOSE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_timer(Timer)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
