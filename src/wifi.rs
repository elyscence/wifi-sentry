pub struct BeaconData {
    pub ssid: String,
    pub channel: Option<u8>,
    pub encryption: String,
}

impl BeaconData {
    pub fn frequency_mhz(&self) -> Option<i32> {
        self.channel.and_then(|ch| {
            match ch {
                1..=13 => Some(2407 + (ch as i32 * 5)),
                14 => Some(2484),
                36..=165 => Some(5000 + (ch as i32 * 5)),
                _ => None,
            }
        })
    }
}

pub fn parse_beacon_frame(frame: &[u8]) -> BeaconData {
    let mut ssid = String::from("Unknown");
    let mut channel = None;
    let mut encryption = String::from("Open");

    let mut offset = 36;

    while offset + 2 <= frame.len() {
        let id = frame[offset];
        let len = frame[offset + 1] as usize;
        
        if offset + 2 + len > frame.len() { break; }
        
        let data = &frame[offset + 2 .. offset + 2 + len];

        match id {
            0 => {
                let raw_ssid = String::from_utf8_lossy(data).to_string();
                let clean_ssid = raw_ssid.trim_matches(char::from(0)).trim();

                if ssid == "Unknown" || (ssid == "<Hidden>" && !clean_ssid.is_empty()) {
                    ssid = if clean_ssid.is_empty() {
                        String::from("<Hidden>")
                    } else {
                        clean_ssid.to_string()
                    };
                }
            }
            3 if len == 1 => {
                channel = Some(data[0]);
            }
            48 => {
                encryption = String::from("WPA2/WPA3");
            }
            221 if len >= 4 => {
                if &data[0..4] == [0x00, 0x50, 0xF2, 0x01] {
                    if encryption == "Open" {
                        encryption = String::from("WPA");
                    }
                }
            }
            _ => {}
        }
        offset += 2 + len;
    }

    BeaconData { ssid, channel, encryption }
}