//! VGM delay command generation

/// VGM delay commands
pub mod cmd {
    /// Wait n samples (16-bit)
    pub const WAIT_NNNN: u8 = 0x61;
    /// Wait 735 samples (1/60 second at 44100Hz)
    pub const WAIT_60TH: u8 = 0x62;
    /// Wait 882 samples (1/50 second at 44100Hz)
    pub const WAIT_50TH: u8 = 0x63;
    /// End of sound data
    pub const END: u8 = 0x66;
    /// Wait n+1 samples (n = 0-15, command 0x70-0x7F)
    pub const WAIT_N_BASE: u8 = 0x70;
}

/// Generate optimal delay commands for a given duration
///
/// Returns a vector of bytes representing the VGM commands
pub fn generate_delay(mut duration: u64) -> Vec<u8> {
    let mut commands = Vec::new();

    while duration > 0 {
        if (735..=751).contains(&duration)
            || duration == 1470
            || duration == 1617
            || (65536..=67152).contains(&duration)
        {
            // Use 1/60 second wait (735 samples)
            commands.push(cmd::WAIT_60TH);
            duration -= 735;
        } else if (882..=898).contains(&duration)
            || duration == 1764
            || (67153..=67299).contains(&duration)
        {
            // Use 1/50 second wait (882 samples)
            commands.push(cmd::WAIT_50TH);
            duration -= 882;
        } else if duration <= 16 {
            // Use short wait (1-16 samples)
            commands.push(cmd::WAIT_N_BASE + (duration as u8) - 1);
            break;
        } else if duration <= 32 {
            // Use max short wait (16 samples)
            commands.push(cmd::WAIT_N_BASE + 15);
            duration -= 16;
        } else if duration <= 65535 {
            // Use 16-bit wait
            commands.push(cmd::WAIT_NNNN);
            commands.push((duration & 0xFF) as u8);
            commands.push(((duration >> 8) & 0xFF) as u8);
            break;
        } else {
            // Use max 16-bit wait
            commands.push(cmd::WAIT_NNNN);
            commands.push(0xFF);
            commands.push(0xFF);
            duration -= 65535;
        }
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_delay() {
        let cmds = generate_delay(5);
        assert_eq!(cmds, vec![0x74]); // 0x70 + 4
    }

    #[test]
    fn test_60th_delay() {
        let cmds = generate_delay(735);
        assert_eq!(cmds, vec![0x62]);
    }

    #[test]
    fn test_50th_delay() {
        let cmds = generate_delay(882);
        assert_eq!(cmds, vec![0x63]);
    }

    #[test]
    fn test_16bit_delay() {
        let cmds = generate_delay(1000);
        assert_eq!(cmds, vec![0x61, 0xE8, 0x03]); // 1000 = 0x03E8
    }
}
