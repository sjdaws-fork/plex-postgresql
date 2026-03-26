use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputValidationMode {
    Off,
    Sample,
    All,
}

pub(crate) const VALIDATE_MODE_ENV: &str = "PLEX_PG_VALIDATE_OUTPUT";
pub(crate) const VALIDATE_SAMPLE_PCT_ENV: &str = "PLEX_PG_VALIDATE_OUTPUT_SAMPLE_PCT";
pub(crate) const DEFAULT_VALIDATE_SAMPLE_PCT: u8 = 5;

pub(crate) fn parse_output_validation_mode(raw: Option<&str>) -> OutputValidationMode {
    match raw.unwrap_or("off").trim().to_ascii_lowercase().as_str() {
        "all" => OutputValidationMode::All,
        "sample" => OutputValidationMode::Sample,
        _ => OutputValidationMode::Off,
    }
}

pub(crate) fn parse_sample_pct(raw: Option<&str>) -> u8 {
    raw.and_then(|s| s.trim().parse::<u8>().ok())
        .map(|v| v.min(100))
        .unwrap_or(DEFAULT_VALIDATE_SAMPLE_PCT)
}

fn stable_hash64(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in input.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn should_validate_output(
    mode: OutputValidationMode,
    translated_sql: &str,
    sample_pct: u8,
) -> bool {
    match mode {
        OutputValidationMode::Off => false,
        OutputValidationMode::All => true,
        OutputValidationMode::Sample => {
            if sample_pct == 0 {
                return false;
            }
            if sample_pct >= 100 {
                return true;
            }
            (stable_hash64(translated_sql) % 100) < u64::from(sample_pct)
        }
    }
}

pub(crate) fn validate_postgres_output(sql: &str) -> Result<(), String> {
    let dialect = PostgreSqlDialect {};
    Parser::parse_sql(&dialect, sql)
        .map(|_| ())
        .map_err(|e| format!("postgres validation error: {e}"))
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn validation_output__output_validation_mode_defaults_to_off() {
        assert_eq!(
            parse_output_validation_mode(None),
            OutputValidationMode::Off
        );
        assert_eq!(
            parse_output_validation_mode(Some("")),
            OutputValidationMode::Off
        );
        assert_eq!(
            parse_output_validation_mode(Some("invalid")),
            OutputValidationMode::Off
        );
    }

    #[test]
    fn validation_output__output_validation_mode_parses_values() {
        assert_eq!(
            parse_output_validation_mode(Some("off")),
            OutputValidationMode::Off
        );
        assert_eq!(
            parse_output_validation_mode(Some("sample")),
            OutputValidationMode::Sample
        );
        assert_eq!(
            parse_output_validation_mode(Some("all")),
            OutputValidationMode::All
        );
        assert_eq!(
            parse_output_validation_mode(Some("  ALL  ")),
            OutputValidationMode::All
        );
    }

    #[test]
    fn validation_output__output_validation_sample_pct_defaults_and_bounds() {
        assert_eq!(parse_sample_pct(None), DEFAULT_VALIDATE_SAMPLE_PCT);
        assert_eq!(parse_sample_pct(Some("")), DEFAULT_VALIDATE_SAMPLE_PCT);
        assert_eq!(parse_sample_pct(Some("7")), 7);
        assert_eq!(parse_sample_pct(Some("255")), 100);
        assert_eq!(parse_sample_pct(Some("bad")), DEFAULT_VALIDATE_SAMPLE_PCT);
    }

    #[test]
    fn validation_output__output_validation_sampling_decision_respects_mode() {
        let sql = "SELECT 1";
        assert!(!should_validate_output(OutputValidationMode::Off, sql, 100));
        assert!(should_validate_output(OutputValidationMode::All, sql, 0));
        assert!(!should_validate_output(
            OutputValidationMode::Sample,
            sql,
            0
        ));
        assert!(should_validate_output(
            OutputValidationMode::Sample,
            sql,
            100
        ));
    }

    #[test]
    fn validation_output__output_validation_postgres_parser_accepts_valid_sql() {
        assert!(validate_postgres_output("SELECT 1").is_ok());
    }
}
