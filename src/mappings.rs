use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// 153 in hex is 99
// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "99";

pub const ROW_COUNT: usize = 3;
pub const COL_COUNT: usize = 6;
pub const KEY_COUNT: usize = ROW_COUNT * COL_COUNT;
pub const ENCODER_COUNT: usize = 0;

pub const IMAGE_FORMAT: ImageFormat = ImageFormat {
    mode: ImageMode::JPEG,
    size: (85, 85),
    rotation: ImageRotation::Rot90,
    mirror: ImageMirroring::Both,
};

#[derive(Debug, Clone)]
pub enum Kind {
    HSV293S,
    AKP153,
    AKP153E,
    AKP153R,
    GK150K,
    RMV01
}

pub const AJAZZ_VID: u16 = 0x0300;
pub const MIRABOX_VID: u16 = 0x5548;
pub const MADDOG_VID: u16 = 0x0c00;
pub const RISEMODE_VID: u16 = 0x0a00;

pub const HSV293S_PID: u16 = 0x6670;

pub const AKP153_PID: u16 = 0x6674;
pub const AKP153E_PID: u16 = 0x1010;
pub const AKP153R_PID: u16 = 0x1020;

pub const GK150K_PID: u16 = 0x1000;

pub const RMV01_PID: u16 = 0x1001;

// Map all queries to usage page 65440 and usage id 1 for now
pub const HSV293S_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_VID, HSV293S_PID);
pub const AKP153_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153_PID);
pub const AKP153E_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153E_PID);
pub const AKP153R_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153R_PID);
pub const GK150K_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MADDOG_VID, GK150K_PID);
pub const RMV01_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, RISEMODE_VID, RMV01_PID);

pub const QUERIES: [DeviceQuery; 6] = [
    HSV293S_QUERY,
    AKP153_QUERY,
    AKP153E_QUERY,
    AKP153R_QUERY,
    GK150K_QUERY,
    RMV01_QUERY,
];

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        match vid {
            AJAZZ_VID => match pid {
                AKP153_PID => Some(Kind::AKP153),
                AKP153E_PID => Some(Kind::AKP153E),
                AKP153R_PID => Some(Kind::AKP153R),
                _ => None,
            },

            MIRABOX_VID => match pid {
                HSV293S_PID => Some(Kind::HSV293S),
                _ => None,
            },

            MADDOG_VID => match pid {
                GK150K_PID => Some(Kind::GK150K),
                _ => None,
            },

            RISEMODE_VID => match pid {
                RMV01_PID => Some(Kind::RMV01),
                _ => None,
            },

            _ => None,
        }
    }

    /// Returns true for devices that emitting two events per key press, instead of one
    /// Currently none of the devices from this family support that
    pub fn supports_both_states(&self) -> bool {
        false
    }

    pub fn is_v2(&self) -> bool {
        false // In the future there may be "v2" devices, so lay some groundwork
    }

    /// There is no point relying on manufacturer/device names reported by the USB stack,
    /// so we return custom names for all the kinds of devices
    pub fn human_name(&self) -> String {
        match &self {
            Self::AKP153 => "Ajazz AKP153",
            Self::AKP153E => "Ajazz AKP153E",
            Self::AKP153R => "Ajazz AKP153R",
            Self::HSV293S => "Mirabox HSV293S",
            Self::GK150K => "Mad Dog GK150K",
            Self::RMV01 => "Risemode Vision 01",
        }
        .to_string()
    }

    /// Because "v1" devices all share the same serial number, use custom suffix to be able to connect
    /// two devices with the different revisions at the same time
    pub fn id_suffix(&self) -> String {
        match &self {
            Self::AKP153 => "153",
            Self::AKP153E => "153E",
            Self::AKP153R => "153R",
            Self::HSV293S => "293S",
            Self::GK150K => "GK150K",
            Self::RMV01 => "RMV01",
        }
        .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub kind: Kind,
}
