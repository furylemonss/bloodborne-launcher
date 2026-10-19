pub const BASE_PKG_URL: &str =
    "https://archive.org/download/Bloodborne_GOTY_Patch_PKGS/Bloodborne.Game.of.the.Year.Edition.PS4-PRELUDE.pkg";
pub const BASE_PKG_NAME: &str = "Bloodborne.Game.of.the.Year.Edition.PS4-PRELUDE.pkg";
pub const BASE_PKG_SIZE: u64 = 31_351_111_680;

pub const UPDATE_PKG_URL: &str =
    "https://archive.org/download/Bloodborne_GOTY_Patch_PKGS/CUSA03173-EUR-Update-v1.09-PRELUDE-%5BPS4ID%5D.pkg";
pub const UPDATE_PKG_NAME: &str = "CUSA03173-EUR-Update-v1.09-PRELUDE-[PS4ID].pkg";
pub const UPDATE_PKG_SIZE: u64 = 181_338_112;

pub const PORT_ZIP_URL: &str =
    "https://github.com/Supermedo/bloodborne_pc/releases/latest/download/Bloodborne-Windows.zip";
pub const PORT_ZIP_NAME: &str = "Bloodborne-Windows.zip";
pub const PORT_ZIP_SIZE: u64 = 121_911_030;

pub const EXTRACTOR_ZIP_URL: &str =
    "https://github.com/AzaharPlus/shadPS4Plus/releases/download/PKG_EXTRACTOR_1_1/ShadPs4Plus-PkgExtractor-1.1-windows.zip";
pub const EXTRACTOR_ZIP_NAME: &str = "ShadPs4Plus-PkgExtractor-1.1-windows.zip";
pub const EXTRACTOR_ZIP_SIZE: u64 = 1_026_861;
pub const EXTRACTOR_EXE_REL: &str = "ShadPs4Plus-PkgExtractor-1.1-windows/pkg_extractor.exe";

/// Golden eboot boyutları (manifest ile birlikte)
pub const BASE_EBOOT_SIZE: u64 = 93_827_832;
pub const PATCH_EBOOT_SIZE: u64 = 93_825_704;

/// Varsayılan: hiçbir şey inmemişken ~65 GB. Gerçek gereksinim disktekilere göre düşer.
pub const REQUIRED_FREE_BYTES: u64 = 65 * 1024 * 1024 * 1024;

pub const DOWNLOAD_BASE: &str = "download_base";
pub const DOWNLOAD_UPDATE: &str = "download_update";
pub const DOWNLOAD_PORT: &str = "download_port";
pub const DOWNLOAD_EXTRACTOR: &str = "download_extractor";
pub const EXTRACT_BASE: &str = "extract_base";
pub const EXTRACT_UPDATE: &str = "extract_update";
pub const MERGE_UPDATE: &str = "merge_update";
pub const EXTRACT_PORT: &str = "extract_port";
pub const WRITE_SETTINGS: &str = "write_settings";

pub const STEPS: &[(&str, &str)] = &[
    (DOWNLOAD_BASE, "Oyun PKG indiriliyor"),
    (DOWNLOAD_UPDATE, "Update PKG indiriliyor"),
    (DOWNLOAD_PORT, "Port indiriliyor"),
    (DOWNLOAD_EXTRACTOR, "PkgExtractor indiriliyor"),
    (EXTRACT_BASE, "Oyun dosyaları çıkartılıyor"),
    (EXTRACT_UPDATE, "Update çıkartılıyor"),
    (MERGE_UPDATE, "Update uygulanıyor"),
    (EXTRACT_PORT, "Port dosyaları açılıyor"),
    (WRITE_SETTINGS, "Ayarlar yazılıyor"),
];
