mod wifi;
mod db;
mod error;

use pnet::datalink::Channel::Ethernet;
use rusqlite::{Connection};
use pnet::datalink;

use tracing::{info, error};

use error::{WifiMonitorError, Result};
use wifi::{parse_beacon_frame};
use db::{DB_SCHEMA, insert_measurement, get_or_insert_ap_id};

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = "wifi_data.db";
    let conn = Connection::open(db_path)?; 
    conn.execute_batch(DB_SCHEMA)?; 
    info!("База данных успешно инициализирована: {}", db_path);

    let adapter_name = "Qualcomm Atheros AR956x Wireless Network Adapter";
    let interface = find_network_interface(&adapter_name)?;

    let (_tx, mut rx) = match datalink::channel(&interface, Default::default()) {
        Ok(Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err(WifiMonitorError::ChannelCreation("Неподдерживаемый тип канала".to_string(),)),
        Err(e) => return Err(WifiMonitorError::ChannelCreation(format!(
                "Не удалось создать канал: {}",
                e
            ))),
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

                let beacon_info = parse_beacon_frame(ieee80211_frame);

                let freq = beacon_info.frequency_mhz();

                let ap_id = match get_or_insert_ap_id(&conn, &bssid_hex, &beacon_info.ssid, beacon_info.channel, freq, &beacon_info.encryption) {
                    Ok(id) => id,
                    Err(e) => {
                        error!("Не удалось получить ID для {}: {}", bssid_hex, e);
                        continue;
                    }
                };

                if let Err(e) = insert_measurement(&conn, ap_id, rssi_dbm) {
                    error!("Ошибка записи уровня сигнала для {}: {}", bssid_hex, e);
                }

                println!("📡 Данные сохранены: BSSID {} | SSID {} | RSSI {} dBm", bssid_hex, &beacon_info.ssid, rssi_dbm);
                println!("📏 Длина RadioTap: {} байт", radiotap_len);
                println!("📡 RSSI (Уровень сигнала): {} dBm", rssi_dbm)

            },
            Err(e) => {
                error!("Ошибка: {}", WifiMonitorError::PacketParsing(e.to_string()));
                continue;
            }
        }
    }
}

fn find_network_interface(keyword: &str) -> Result<pnet::datalink::NetworkInterface> {
    let interfaces = datalink::interfaces();
    
    interfaces
        .into_iter()
        .find(|i| i.description.contains(keyword))
        .ok_or_else(|| WifiMonitorError::AdapterNotFound(keyword.to_string()))
}