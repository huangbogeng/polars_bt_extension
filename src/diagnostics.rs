#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Info,
    Warn,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
        }
    }
}

pub fn verbose_enabled() -> bool {
    let config = polars_config::config();
    config.reload_env_var("POLARS_VERBOSE");
    config.verbose()
}

pub fn render_record(
    component: &str,
    level: Level,
    event: &str,
    fields: &[(&str, String)],
) -> String {
    let mut output = format!(
        "[polars-bt][{component}][{}] event={event}",
        level.as_str()
    );
    for (key, value) in fields {
        output.push(' ');
        output.push_str(key);
        output.push('=');
        output.push_str(value);
    }
    output
}

pub fn emit(component: &str, level: Level, event: &str, fields: &[(&str, String)]) {
    eprintln!("{}", render_record(component, level, event, fields));
}

#[cfg(test)]
mod tests {
    use super::{render_record, Level};

    #[test]
    fn renders_deterministic_key_value_record() {
        assert_eq!(
            render_record(
                "mosaic",
                Level::Warn,
                "halt",
                &[
                    ("reason", "NEGATIVE_CASH".into()),
                    ("day_index", "7".into()),
                ],
            ),
            "[polars-bt][mosaic][WARN] event=halt reason=NEGATIVE_CASH day_index=7"
        );
    }
}
