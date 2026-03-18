use rusqlite::{params, Transaction};

#[derive(Debug, Clone)]
pub struct TagGeneratorSettings {
    pub prefix: String,
    pub number_width: usize,
    pub separator: String,
}

pub fn load_settings(tx: &Transaction<'_>) -> Result<TagGeneratorSettings, rusqlite::Error> {
    tx.query_row(
        "
        SELECT prefix, number_width, separator
        FROM tag_generator_settings
        WHERE id = 1
        ",
        [],
        |row| {
            let width: i64 = row.get(1)?;
            Ok(TagGeneratorSettings {
                prefix: row.get(0)?,
                number_width: width.max(0) as usize,
                separator: row.get(2)?,
            })
        },
    )
}

pub fn load_global_next_value(tx: &Transaction<'_>) -> Result<i64, rusqlite::Error> {
    tx.query_row(
        "SELECT next_value FROM tag_generator_counters WHERE id = 1",
        [],
        |row| row.get(0),
    )
}

pub fn persist_global_next_value(
    tx: &Transaction<'_>,
    next_value: i64,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "
        UPDATE tag_generator_counters
        SET next_value = ?1,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = 1
        ",
        params![next_value],
    )?;

    Ok(())
}

pub fn format_tag(settings: &TagGeneratorSettings, value: i64) -> String {
    let number = format!("{:0width$}", value, width = settings.number_width);

    match (settings.prefix.is_empty(), settings.separator.is_empty()) {
        (true, _) => number,
        (false, true) => format!("{}{}", settings.prefix, number),
        (false, false) => format!("{}{}{}", settings.prefix, settings.separator, number),
    }
}
