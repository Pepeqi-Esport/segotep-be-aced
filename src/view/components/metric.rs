//! Display metrics selectable on the BeAced dashboard.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    CpuTemp = 1,
    CpuLoad = 2,
    CpuClock = 3,
    CpuPower = 4,
    GpuTemp = 5,
    GpuLoad = 6,
    GpuClock = 7,
    GpuPower = 8,
}

impl Metric {
    pub fn all() -> [Metric; 8] {
        [
            Self::CpuTemp,
            Self::CpuLoad,
            Self::CpuClock,
            Self::CpuPower,
            Self::GpuTemp,
            Self::GpuLoad,
            Self::GpuClock,
            Self::GpuPower,
        ]
    }

    pub fn bit(self) -> u8 {
        1 << (self as u8 - 1)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CpuTemp => "CPU Temperature",
            Self::CpuLoad => "CPU Load",
            Self::CpuClock => "CPU Clock",
            Self::CpuPower => "CPU Power",
            Self::GpuTemp => "GPU Temperature",
            Self::GpuLoad => "GPU Load",
            Self::GpuClock => "GPU Clock",
            Self::GpuPower => "GPU Power",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::CpuTemp => "Temp",
            Self::CpuLoad => "Load",
            Self::CpuClock => "Clock",
            Self::CpuPower => "Power",
            Self::GpuTemp => "Temp",
            Self::GpuLoad => "Load",
            Self::GpuClock => "Clock",
            Self::GpuPower => "Power",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Self::CpuTemp | Self::CpuLoad | Self::CpuClock | Self::CpuPower => "CPU",
            _ => "GPU",
        }
    }
}
