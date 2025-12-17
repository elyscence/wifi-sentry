use rusqlite::{Connection, Result, params};
use chrono::Utc;

pub const DB_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS access_points (
        id INTEGER PRIMARY KEY,
        bssid TEXT NOT NULL UNIQUE,
        ssid TEXT,
        channel INTEGER,
        frequency_mhz INTEGER,
        encryption TEXT,
        vendor TEXT,
        first_seen DATETIME NOT NULL
    );

    CREATE TABLE IF NOT EXISTS measurements (
        id INTEGER PRIMARY KEY,
        ap_id INTEGER NOT NULL,
        timestamp DATETIME NOT NULL,
        rssi_dbm INTEGER NOT NULL,
        FOREIGN KEY (ap_id) REFERENCES access_points (id)
    );
    CREATE INDEX IF NOT EXISTS idx_measurement_time ON measurements (timestamp);
    CREATE INDEX IF NOT EXISTS idx_measurement_ap ON measurements (ap_id);
";

pub fn get_or_insert_ap_id(conn: &Connection, bssid: &str, ssid: &str, channel: Option<u8>, frequency_mhz: Option<i32>, encryption: &str) -> Result<i64> {
    match conn.query_row(
        "SELECT id FROM access_points WHERE bssid = ?",
        params![bssid],
        |row| row.get(0),
    ) {
        Ok(id) => {
            conn.execute(
                "UPDATE access_points SET ssid = ?1, channel = ?2, frequency_mhz = ?3, encryption = ?4 WHERE id = ?5", 
                params![ssid, channel, frequency_mhz, encryption, id]
            )?;
            Ok(id)
        },

        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let now = Utc::now().to_string();

            conn.execute(
                "INSERT INTO access_points (bssid, first_seen, ssid, channel, frequency_mhz) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![bssid, now, ssid.is_empty().then_some("Unknown").unwrap_or(&ssid), channel, frequency_mhz],
            )?;

            Ok(conn.last_insert_rowid())

            }

        Err(e) => return Err(e),
    }
}

pub fn insert_measurement(conn: &Connection, ap_id: i64, rssi_dbm: i8) -> Result<()> {
    let now = Utc::now().to_string();

    conn.execute(
        "INSERT INTO measurements (ap_id, timestamp, rssi_dbm) VALUES (?1, ?2, ?3)",
        params![ap_id, now, rssi_dbm]
    )?;

    Ok(())
}