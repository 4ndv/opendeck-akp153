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

#[derive(Debug, Clone)]
pub enum Kind {
    HSV293S,
    HSV293SV3,
    HSV293SV3_1005,
    AKP153,
    AKP153E,
    AKP153EREV2,
    AKP153R,
    MSDONE,
    GK150K,
    RMV01,
    SFSTC,
    TMICESC,
}

pub const AJAZZ_VID: u16 = 0x0300;
pub const MIRABOX_VID: u16 = 0x5548;
pub const MIRABOX_2_VID: u16 = 0x6603;
pub const MG_VID: u16 = 0x0b00;
pub const MADDOG_VID: u16 = 0x0c00;
pub const RISEMODE_VID: u16 = 0x0a00;
pub const SF_STC_VID: u16 = 0x1500;
pub const TMICE_VID: u16 = 0x0500;

pub const HSV293S_PID: u16 = 0x6670;
pub const HSV293SV3_PID: u16 = 0x1014;
pub const HSV293SV3_1005_PID: u16 = 0x1005;

pub const AKP153_PID: u16 = 0x6674;
pub const AKP153E_PID: u16 = 0x1010;
pub const AKP153E_REV2_PID: u16 = 0x3010;
pub const AKP153R_PID: u16 = 0x1020;

pub const MSD_ONE_PID: u16 = 0x1000;

pub const GK150K_PID: u16 = 0x1000;

pub const RMV01_PID: u16 = 0x1001;
pub const SF_STC_PID: u16 = 0x3003;
pub const TMICESC_PID: u16 = 0x1001;

// Map all queries to usage page 65440 and usage id 1 for now
pub const HSV293S_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_VID, HSV293S_PID);
pub const HSV293SV3_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_2_VID, HSV293SV3_PID);
pub const HSV293SV3_1005_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_2_VID, HSV293SV3_1005_PID);
pub const AKP153_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_VID, AKP153_PID);
pub const AKP153E_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153E_PID);
pub const AKP153E_REV2_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153E_REV2_PID);
pub const AKP153R_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP153R_PID);
pub const MSD_ONE_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MG_VID, MSD_ONE_PID);
pub const GK150K_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MADDOG_VID, GK150K_PID);
pub const RMV01_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, RISEMODE_VID, RMV01_PID);
pub const SF_STC_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, SF_STC_VID, SF_STC_PID);
pub const TMICESC_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, TMICE_VID, TMICESC_PID);

pub const QUERIES: [DeviceQuery; 12] = [
    HSV293S_QUERY,
    HSV293SV3_QUERY,
    HSV293SV3_1005_QUERY,
    AKP153_QUERY,
    AKP153E_QUERY,
    AKP153E_REV2_QUERY,
    AKP153R_QUERY,
    MSD_ONE_QUERY,
    GK150K_QUERY,
    RMV01_QUERY,
    SF_STC_QUERY,
    TMICESC_QUERY,
];

/// Returns correct image format for device kind and key
pub fn get_image_format_for_key(kind: &Kind, key: u8) -> ImageFormat {
    if kind.protocol_version() == 1 {
        return ImageFormat {
            mode: ImageMode::JPEG,
            size: (85, 85),
            rotation: ImageRotation::Rot90,
            mirror: ImageMirroring::Both,
        };
    }

    let size = match key {
        5 | 11 | 17 => (82, 82),
        _ => (95, 95),
    };

    ImageFormat {
        mode: ImageMode::JPEG,
        size,
        rotation: ImageRotation::Rot90,
        mirror: ImageMirroring::Both,
    }
}

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        match vid {
            AJAZZ_VID => match pid {
                AKP153E_PID => Some(Kind::AKP153E),
                AKP153E_REV2_PID => Some(Kind::AKP153EREV2),
                AKP153R_PID => Some(Kind::AKP153R),
                _ => None,
            },

            MIRABOX_VID => match pid {
                AKP153_PID => Some(Kind::AKP153),
                HSV293S_PID => Some(Kind::HSV293S),
                _ => None,
            },

            MIRABOX_2_VID => match pid {
                HSV293SV3_PID => Some(Kind::HSV293SV3),
                HSV293SV3_1005_PID => Some(Kind::HSV293SV3_1005),
                _ => None,
            },

            MG_VID => match pid {
                MSD_ONE_PID => Some(Kind::MSDONE),
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

            SF_STC_VID => match pid {
                SF_STC_PID => Some(Kind::SFSTC),
                _ => None,
            },

            TMICE_VID => match pid {
                TMICESC_PID => Some(Kind::TMICESC),
                _ => None,
            },

            _ => None,
        }
    }

    /// Returns protocol version for device
    pub fn protocol_version(&self) -> usize {
        match self {
            Self::HSV293SV3 => 3,
            Self::HSV293SV3_1005 => 3,
            Self::AKP153EREV2 => 3,
            Self::SFSTC => 3,
            _ => 1,
        }
    }

    /// There is no point relying on manufacturer/device names reported by the USB stack,
    /// so we return custom names for all the kinds of devices
    pub fn human_name(&self) -> String {
        match &self {
            Self::HSV293S => "Mirabox HSV293S",
            Self::HSV293SV3 => "Mirabox HSV293SV3",
            Self::HSV293SV3_1005 => "Mirabox HSV293SV3",
            Self::AKP153 => "Ajazz AKP153",
            Self::AKP153E => "Ajazz AKP153E",
            Self::AKP153EREV2 => "Ajazz AKP153E (rev. 2)",
            Self::AKP153R => "Ajazz AKP153R",
            Self::MSDONE => "Mars Gaming MSD-ONE",
            Self::GK150K => "Mad Dog GK150K",
            Self::RMV01 => "Risemode Vision 01",
            Self::SFSTC => "Soomfon Stream Controller",
            Self::TMICESC => "TMICE Stream Controller",
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
            Self::MSDONE => "MSDONE",
            Self::GK150K => "GK150K",
            Self::RMV01 => "RMV01",
            Self::TMICESC => "TMICESC",
            // This method would not be called for "v2"/"v3" devices, so mark them as unreachable
            Self::HSV293SV3 => unreachable!(),
            Self::HSV293SV3_1005 => unreachable!(),
            Self::AKP153EREV2 => unreachable!(),
            Self::SFSTC => unreachable!(),
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
