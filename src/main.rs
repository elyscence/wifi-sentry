use pnet::datalink::Channel::Ethernet;
use rusqlite::{Connection, Result, params};
use pnet::datalink;
use chrono::Utc;

const DB_SCHEMA: &str = "
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

fn get_or_insert_ap_id(conn: &Connection, bssid: &str, ssid: &str, channel: Option<u8>, frequency_mhz: Option<i32>) -> Result<i64> {
    match conn.query_row(
        "SELECT id FROM access_points WHERE bssid = ?",
        params![bssid],
        |row| row.get(0),
    ) {
        Ok(id) => {
            conn.execute(
                "UPDATE access_points SET ssid = ?1, channel = ?2, frequency_mhz = ?3 WHERE id = ?4", 
                params![ssid, channel, frequency_mhz, id]
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

fn insert_measurement(conn: &Connection, ap_id: i64, rssi_dbm: i8) -> Result<()> {
    let now = Utc::now().to_string();

    conn.execute(
        "INSERT INTO measurements (ap_id, timestamp, rssi_dbm) VALUES (?1, ?2, ?3)",
        params![ap_id, now, rssi_dbm]
    )?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Инициализация БД
    let db_path = "wifi_data.db";
    let conn = Connection::open(db_path)?; 
    conn.execute_batch(DB_SCHEMA)?; 
    println!("База данных успешно инициализирована: {}", db_path);

    let interfaces = datalink::interfaces();
    let adapter_name = "Qualcomm Atheros AR956x Wireless Network Adapter";
    let interface = interfaces.into_iter()
        .find(|i| i.description.contains(adapter_name))
        .expect("Проводной адаптер Realtek не найден в списке!");

    let (mut tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Не удалось создать канал"),
        Err(e) => panic!("Ошибка при открытии канала: {:?}", e),
        _ => panic!("Получен неожиданный тип канала"),
    };

    loop {
        match rx.next() {
            Ok(packet) => {
                if packet.len() < 4 { continue; } 
            
                let len_bytes: [u8; 2] = [packet[2], packet[3]]; 
                let radiotap_len = u16::from_le_bytes(len_bytes); 

                let rssi_u8 = packet[8];
                let rssi_dbm = rssi_u8 as i8;

                let ieee80211_frame = &packet[radiotap_len as usize..];
                if ieee80211_frame.len() < 22 { continue; }
                if ieee80211_frame[0] != 0x80 { continue; }

                let bssid_bytes: &[u8] = &ieee80211_frame[16..22];

                let bssid_hex = bssid_bytes.iter()
                    .map(|byte| format!("{:02X}", byte))
                    .collect::<Vec<_>>()
                    .join(":");

                let mut ie_start_offset = 36;

                let mut ssid = String::from("Unknown");
                let mut channel: Option<u8> = None;

                while ie_start_offset + 2 <= ieee80211_frame.len() {
                    let element_id = ieee80211_frame[ie_start_offset];
                    let element_lenght = ieee80211_frame[ie_start_offset + 1] as usize;

                    if ie_start_offset + 2 + element_lenght > ieee80211_frame.len() {
                        break;
                    }

                    if element_id == 0 {
                        let ssid_name = &ieee80211_frame[ie_start_offset + 2 .. ie_start_offset + 2 + element_lenght];
                        let raw_ssid = String::from_utf8_lossy(ssid_name).into_owned();

                        if !raw_ssid.trim_matches(char::from(0)).trim().is_empty() {
                            ssid = raw_ssid;
                        }
                    } else if element_id == 3 && element_lenght == 1 {
                        channel = Some(ieee80211_frame[ie_start_offset + 2]);
                    }

                    if ssid != "Unknown" && channel.is_some() {
                        break; 
                    }

                    ie_start_offset += 2 + element_lenght;
                }

                let mut frequency_mhz: Option<i32> = None;

                if let Some(ch) = channel {
                    frequency_mhz = match ch {
                        1..=13 => Some(2407 + (ch as i32 * 5)),
                        14 => Some(2484),
                        36 => Some(5180), 
                        _ => None,
                    };
                }

                let ap_id = get_or_insert_ap_id(&conn, &bssid_hex, &ssid, channel, frequency_mhz)?;

                insert_measurement(&conn, ap_id, rssi_dbm)?;
                println!("📡 Данные сохранены: BSSID {} | SSID {} | RSSI {} dBm", bssid_hex, ssid, rssi_dbm);

                
                println!("📏 Длина RadioTap: {} байт", radiotap_len);
                println!("📡 RSSI (Уровень сигнала): {} dBm", rssi_dbm)

            },
            Err(e) => {
                panic!("An error occured: {}", e)
            }
        }
    }
}