//! Sensor label mapping (mirrors official `config` key/value lines).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorMapping {
    pub cpu_temperature: String,
    pub cpu_load: String,
    pub cpu_clock: String,
    pub cpu_power: String,
    pub gpu_temperature: String,
    pub gpu_load: String,
    pub gpu_clock: String,
    pub gpu_power: String,
}

impl Default for SensorMapping {
    fn default() -> Self {
        // Defaults mirror official/config + configInit labels.
        Self {
            cpu_temperature: "Core Max".into(),
            cpu_load: "Total CPU Usage".into(),
            cpu_clock: String::new(),
            cpu_power: "CPU Package Power".into(),
            gpu_temperature: String::new(),
            gpu_load: String::new(),
            gpu_clock: String::new(),
            gpu_power: String::new(),
        }
    }
}

impl SensorMapping {
    /// Apply official `config` file (`Key: Value` per line).
    pub fn apply_official_config(&mut self, text: &str) {
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim().to_string();
            match key.trim() {
                "CpuTemperature" => self.cpu_temperature = value,
                "CpuLoad" => self.cpu_load = value,
                "CpuClock" => self.cpu_clock = value,
                "CpuPower" => self.cpu_power = value,
                "GpuTemperature" => self.gpu_temperature = value,
                "GpuLoad" => self.gpu_load = value,
                "GpuClock" => self.gpu_clock = value,
                "GpuPower" => self.gpu_power = value,
                _ => {}
            }
        }
    }
}
