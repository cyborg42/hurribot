use time::{macros::offset, OffsetDateTime};

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

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(non_blocking)
        .with_span_events(FmtSpan::CLOSE)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_timer(Timer)
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    tracing::info!("tttttt");
    guard
}

pub fn local_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(offset!(+8))
}
pub fn file_logger(name: &str) -> tracing_appender::non_blocking::WorkerGuard {
    let name = if name.is_empty() {
        "".to_string()
    } else {
        name.to_string() + "_"
    };
    let parse_str = format!(
        "hurribot_{}[year]-[month]-[day]T[hour]:[minute]:[second]",
        name
    );
    let log_name = local_now()
        .format(&time::format_description::parse(&parse_str).unwrap())
        .unwrap();
    init_log(&log_name)
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

pub fn truncate_step(value: f64, step: f64) -> f64 {
    (value / step).trunc() * step
}