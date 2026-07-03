use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStep {
    pub cmd: String,
    pub delay_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub name: String,
    pub command: String,
    pub terminal: String,
    pub auto_start: bool,
    pub group_name: String,
    pub steps: Vec<CommandStep>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn new(app_data_dir: &Path) -> Result<Self, String> {
        let db_path = app_data_dir.join("openstart.db");
        let conn =
            Connection::open(&db_path).map_err(|e| format!("Failed to open database: {}", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS commands (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                terminal TEXT NOT NULL,
                auto_start INTEGER NOT NULL DEFAULT 0,
                group_name TEXT NOT NULL DEFAULT '',
                steps TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|e| format!("Failed to create table: {}", e))?;

        // Migration: add group_name column if missing (for DBs created before this field existed)
        let _ = conn.execute_batch(
            "ALTER TABLE commands ADD COLUMN group_name TEXT NOT NULL DEFAULT '';",
        );

        // Migration: add steps column if missing
        let _ = conn.execute_batch(
            "ALTER TABLE commands ADD COLUMN steps TEXT NOT NULL DEFAULT '';",
        );

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list_commands(&self) -> Result<Vec<Command>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let mut stmt = conn
            .prepare("SELECT id, name, command, terminal, auto_start, group_name, steps, created_at, updated_at FROM commands ORDER BY group_name, created_at DESC")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Command {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    terminal: row.get(3)?,
                    auto_start: row.get::<_, i32>(4)? != 0,
                    group_name: row.get(5)?,
                    steps: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("Failed to query commands: {}", e))?;

        let mut commands = Vec::new();
        for row in rows {
            commands.push(row.map_err(|e| format!("Failed to read row: {}", e))?);
        }
        Ok(commands)
    }

    pub fn get_command(&self, id: &str) -> Result<Command, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.query_row(
            "SELECT id, name, command, terminal, auto_start, group_name, steps, created_at, updated_at FROM commands WHERE id = ?1",
            params![id],
            |row| {
                Ok(Command {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    terminal: row.get(3)?,
                    auto_start: row.get::<_, i32>(4)? != 0,
                    group_name: row.get(5)?,
                    steps: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .map_err(|e| format!("Failed to get command '{}': {}", id, e))
    }

    pub fn add_command(&self, cmd: &Command) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.execute(
            "INSERT INTO commands (id, name, command, terminal, auto_start, group_name, steps, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                cmd.id, cmd.name, cmd.command, cmd.terminal,
                cmd.auto_start as i32, cmd.group_name,
                serde_json::to_string(&cmd.steps).unwrap_or_default(),
                cmd.created_at, cmd.updated_at,
            ],
        )
        .map_err(|e| format!("Failed to add command: {}", e))?;
        Ok(())
    }

    pub fn update_command(&self, cmd: &Command) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let updated_at = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE commands SET name=?1, command=?2, terminal=?3, auto_start=?4, group_name=?5, steps=?6, updated_at=?7 WHERE id=?8",
                params![
                    cmd.name, cmd.command, cmd.terminal,
                    cmd.auto_start as i32, cmd.group_name,
                    serde_json::to_string(&cmd.steps).unwrap_or_default(),
                    updated_at, cmd.id,
                ],
            )
            .map_err(|e| format!("Failed to update command: {}", e))?;

        if affected == 0 {
            return Err(format!("Command '{}' not found", cmd.id));
        }
        Ok(())
    }

    pub fn delete_command(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let affected = conn
            .execute("DELETE FROM commands WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete command: {}", e))?;

        if affected == 0 {
            return Err(format!("Command '{}' not found", id));
        }
        Ok(())
    }

    pub fn get_auto_start_commands(&self) -> Result<Vec<Command>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let mut stmt = conn
            .prepare("SELECT id, name, command, terminal, auto_start, group_name, steps, created_at, updated_at FROM commands WHERE auto_start = 1 ORDER BY created_at DESC")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Command {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    terminal: row.get(3)?,
                    auto_start: true,
                    group_name: row.get(5)?,
                    steps: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("Failed to query auto-start commands: {}", e))?;

        let mut commands = Vec::new();
        for row in rows {
            commands.push(row.map_err(|e| format!("Failed to read row: {}", e))?);
        }
        Ok(commands)
    }

    pub fn delete_group_commands(&self, group: &str) -> Result<usize, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let count = conn
            .execute("DELETE FROM commands WHERE group_name = ?1", params![group])
            .map_err(|e| format!("Failed to delete group commands: {}", e))?;
        Ok(count)
    }

    pub fn get_group_commands(&self, group: &str) -> Result<Vec<Command>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let mut stmt = conn
            .prepare("SELECT id, name, command, terminal, auto_start, group_name, steps, created_at, updated_at FROM commands WHERE group_name = ?1 ORDER BY created_at ASC")
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt
            .query_map(params![group], |row| {
                Ok(Command {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    terminal: row.get(3)?,
                    auto_start: row.get::<_, i32>(4)? != 0,
                    group_name: row.get(5)?,
                    steps: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("Failed to query group commands: {}", e))?;

        let mut commands = Vec::new();
        for row in rows {
            commands.push(row.map_err(|e| format!("Failed to read row: {}", e))?);
        }
        Ok(commands)
    }
}
