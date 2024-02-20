#![allow(dead_code)]
#![allow(unused_imports)]
use time::{OffsetDateTime, UtcOffset};

pub mod candle_chart;
pub mod contract;
pub mod strategy;

pub fn init_log(file_name: &str) -> tracing_appender::non_blocking::WorkerGuard {
    let file_name = file_name.to_owned() + ".log";
    use tracing::Level;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::time::FormatTime;
    use tracing_subscriber::FmtSubscriber;

    struct UtcOffset;
    impl FormatTime for UtcOffset {
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
        .with_timer(UtcOffset)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    _guard
}

pub fn local_now() -> OffsetDateTime {
    time::OffsetDateTime::now_utc().to_offset(UtcOffset::from_hms(8, 0, 0).unwrap())
}
