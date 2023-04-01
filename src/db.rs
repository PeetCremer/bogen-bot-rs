use std::path::Path;

pub use sqlite::{Error, Result};

pub struct SheetDB {
    connection: sqlite::ConnectionWithFullMutex,
}

impl SheetDB {
    pub fn open() -> Result<SheetDB> {
        SheetDB::open_with_path("db.sqlite")
    }

    pub fn open_with_path<T: AsRef<Path>>(path: T) -> Result<SheetDB> {
        let connection = sqlite::Connection::open_with_full_mutex(path)?;

        connection.execute("CREATE TABLE IF NOT EXISTS users (guild_id UNSIGNED BIG INT, author_id UNSIGNED BIG INT, sheet TEXT);")?;
        connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_users ON users(guild_id, author_id, sheet);",
        )?;
        // Every author can only appear once per sheet
        connection
            .execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_unique_author_per_guild ON users(guild_id, author_id);")?;
        // Every sheet can only appear once per guild
        connection.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_unique_sheet_per_guild ON users(guild_id, sheet);")?;

        Ok(SheetDB { connection })
    }

    pub fn get_sheet(&mut self, guild_id: u64, author_id: u64) -> Result<Option<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT sheet FROM users WHERE guild_id=:guild_id AND author_id=:author_id")?;
        statement.bind::<&[(&str, i64)]>(
            &[
                (":guild_id", guild_id as i64),
                (":author_id", author_id as i64),
            ][..],
        )?;

        match statement.next()? {
            sqlite::State::Row => Ok(Some(statement.read::<String, _>("sheet")?)),
            sqlite::State::Done => Ok(None),
        }
    }

    pub fn store_sheet(&mut self, guild_id: u64, author_id: u64, sheet: &str) -> Result<()> {
        let mut statement = self.connection.prepare("INSERT OR REPLACE INTO users (guild_id, author_id, sheet) VALUES (:guild_id, :author_id, :sheet);")?;
        statement.bind::<&[(&str, sqlite::Value)]>(
            &[
                (":guild_id", (guild_id as i64).into()),
                (":author_id", (author_id as i64).into()),
                (":sheet", sheet.into()),
            ][..],
        )?;
        statement.next()?;

        Ok(())
    }
}
