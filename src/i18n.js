const STORAGE_KEY = "bb-lang";

const dict = {
  tr: {
    minimize: "Küçült",
    trayHint: "Tray'e al",
    quit: "Kapat",
    lang: "EN",

    install: "Kurulum",
    prepTitle: "Avcını avına <span>hazırla</span>",
    prepSub:
      "Oyun PKG, update, PkgExtractor ve Bloodborne PC portu tek akışta indirilir. Gerekli boş alan, diskte hazır PKG'lere göre hesaplanır. Kurulum klasörünü seç ve devam et.",
    rootFolder: "Kurulum kök klasörü",
    folderNotChosen: "Klasör seçilmedi",
    change: "Değiştir",
    startInstall: "Kuruluma Başla",
    diskWaiting: "Disk bilgisi bekleniyor",
    freeSpace: "Boş alan",
    required: "Gerekli",
    status: "Durum",
    freeLabel: "boş alan",
    enough: "Yeterli",
    notEnough: "Yetersiz",
    diskFull: "Boş",
    ofNeeded: "/ gerekli alan",
    contentsNote:
      "İçerik: base PKG (~29 GB), update PKG, Supermedo Windows portu ve AzaharPlus PkgExtractor. Dosyalar kaldığın yerden devam eder.",

    flow: "Kurulum akışı",
    galleryDetail: "Detaylı indirme",
    gallerySettings: "Ayarlar",
    galleryPath: "Yolu değiştir",
    openDetailHint: "Detaylı indirme sayfasını aç",

    pending: "Bekliyor",
    runningBadge: "Aktif",
    done: "Tamam",
    skipped: "Atlandı",
    errorBadge: "Hata",

    st_idle: "Hazır başlatılıyor",
    st_running: "İndiriliyor",
    st_paused: "Duraklatıldı",
    st_extracting: "Dosyalar hazırlanıyor",
    st_ready: "Hazır",
    st_error: "Hata",

    step_download_base: "Oyun PKG indiriliyor",
    step_download_update: "Update PKG indiriliyor",
    step_download_port: "Port indiriliyor",
    step_download_extractor: "PkgExtractor indiriliyor",
    step_extract_base: "Oyun dosyaları çıkartılıyor",
    step_extract_update: "Update çıkartılıyor",
    step_merge_update: "Update uygulanıyor",
    step_extract_port: "Port dosyaları açılıyor",
    step_write_settings: "Ayarlar yazılıyor",

    filesUnit: "dosya",
    leftUnit: "kaldı",

    file_base: "Bloodborne GOTY PKG",
    file_update: "CUSA03173 Update v1.09 PKG",
    file_port: "Bloodborne-Windows.zip",
    file_extractor: "PkgExtractor 1.1",
    file_default: "İndiriliyor",

    nextUp: "Sıradaki:",

    detailEyebrow: "Detaylı indirme",
    detailSub: "Adım adım ilerleme. Küçük panelden galeriye dönebilirsin.",
    galleryBack: "Galeri",
    ready: "Hazır",
    pause: "Dondur",
    resume: "Devam",
    pathChange: "Yolu Değiştir",
    settingsBtn: "Ayarlar",
    sourceNote: "Kaynak: archive.org PKG'ler + Supermedo bloodborne_pc",

    readyKicker: "CUSA03173 v1.09",
    readySub: "Kurulum tamam. Hunter's Dream seni bekliyor.",
    readyStatus: "Durum",
    readyOrb: "Oyun dosyaları hazır, port ve ayarlar yerinde.",
    play: "OYNA",
    stop: "DURDUR",
    advancedBtn: "Gelişmiş",
    advancedHint: "Ayarlar / hileler / başarımlar / online paneli",
    advancedStarted: "Bloodborne.exe açıldı",
    advancedErr: "Bloodborne.exe açılamadı",
    cleanTempBtn: "Geçici dosyaları sil",
    cleanTempOnlyReady: "Geçici dosyalar yalnızca tam yükleme sonrası silinebilir",
    gameStopped: "Oyun durduruldu",
    gameStoppedSub: "Play Bloodborne.exe ve Bloodborne.exe kapatıldı",
    recheck: "Yeniden Kontrol",

    settingsTitle: "Depolama & temizlik",
    settingsSub:
      "Ne sileceğini seç. Güvenli olanlar yalnızca artık dosyaları temizler; kırmızı olanlar kurulumu baştan almanı gerektirir.",
    safeClean: "Güvenli temizlik",
    safeCleanSub: "Oyunu bozmaz. Yeniden indirmen gerekmez.",
    partialDl: "Yarım indirmeler (.part)",
    partialDlSub: "Kesilmiş indirmelerin artıkları. Sonraki kurulumda sıfırdan başlar.",
    patchTmp: "Update geçici klasörü",
    patchTmpSub: "game/CUSA03173-patch — update birleştirildikten sonra artık.",
    extractorTool: "PkgExtractor aracını sil",
    extractorToolSub: "tools/ klasörü. Gerekirse update adımında tekrar iner (~1 MB).",
    resetSec: "Kurulumu sıfırla",
    resetSecSub: "Bu dosyalar silinirse yeniden indirmen gerekir.",
    allPkgs: "Tüm PKG ve zip dosyaları",
    allPkgsSub: "~29 GB oyun PKG + update + port + extractor. Tekrar indirme zorunlu.",
    extractedGame: "Çıkartılmış oyun ve port",
    extractedGameSub: "game/ ve port/ silinir. Kurulum baştan alınır (indirmeler durur).",
    installPath: "Kurulum yolu",
    noPath: "yok",
    settingsPathNote:
      "Temizlik bu klasördeki dosyalara uygulanır. Değiştirmek için ana ekrandan “Yolu Değiştir” kullan.",
    cleanBtn: "Seçilenleri Temizle",
    back: "Geri",

    confirmAllPkgs: "Tüm PKG ve zip dosyaları silinecek. Emin misin?",
    confirmExtracted: "Çıkartılmış oyun ve port silinecek. Emin misin?",
    cleanDone: "Temizlik bitti",
    cleanDoneSub: "dosya/klasör temizlendi",
    cleanErr: "Temizlik hatası",

    folderPickTitle: "Kurulum klasörü seç",
    folderUpdated: "Klasör güncellendi",
    folderErr: "Klasör alınamadı",
    diskErr: "Disk bilgisi alınamadı",
    noDisk: "Yetersiz alan",
    noDiskSub: "Boş alan gerekenden az — PKG zaten varsa gereken düşer",
    installStartErr: "Kurulum başlatılamadı",
    checkErr: "Kontrol hatası",
    stateErr: "Durum okunamadı",
    bootErr: "Başlatma hatası",
    errUnknown: "Bilinmeyen hata",
    errTitle: "Hata",
    installDone: "Kurulum tamam",
    installDoneSub: "OYNA butonu aktif",
    readyToast: "Hazır",
    readyToastSub: "Bloodborne oynamaya hazır",
    gameStarted: "Bloodborne başlatıldı",
    gameStartedSub: "Oyun kapanınca seçenekler çıkacak",
    gameOpenErr: "Oyun açılamadı",
    shutdownTitle: "Oyun kapandı",
    shutdownQ: "Şimdi ne yapmak istersin?",
    stayLauncher: "Launcher'da kal",
    shutdownPc: "Bilgisayarı kapat",
    restartPc: "Yeniden başlat",
    shuttingDown: "Kapatılıyor",
    shuttingDownSub: "60 saniye içinde bilgisayar kapanacak",
    shutdownErr: "Kapatma hatası",
    restarting: "Yeniden başlatılıyor",
    restartingSub: "60 saniye içinde yeniden başlayacak",
    restartErr: "Restart hatası",

    sn: "sn",
    min: "dk",
    hour: "sa",
    almost: "az kaldı",

    errDiskTitle: "Disk kapasitesi yetersiz",
    errDiskBody:
      "Kurulum için gerekli disk alanına ulaşamadı. Fazla dosya temizle veya başka diske kur, sonra tekrar dene.",
    errExtractTitle: "Dosyalar çıkartılamadı",
    errExtractBody:
      "PkgExtractor çıkartmayı tamamlayamadı. Dosya bozuk veya hedef klasöre yazma engellenmiş olabilir. Ayarlar → çıkartılmış dosyaları temizle ve kurulumu yeniden başlat.",
    errEbootTitle: "eboot.bin bulunamadı",
    errEbootBody:
      "Çıkartma bitti ama oyunun çalıştırmak için gereklisi eboot.bin yok. PKG eksik/bozuk olabilir. Ayarlar’dan game klasörünü temizleyip tekrar dene.",
    errDownloadTitle: "İndirme başarısız",
    errDownloadBody:
      "PKG veya port dosyası indirilemedi. İnternet bağlantısını kontrol et; yarım dosyalar kaldığında kurulum kaldığı yerden devam eder.",
    errZipTitle: "Arşiv açılamadı",
    errZipBody:
      "Port/zip dosyası okunamadı veya bozuk. Ayarlar’dan port’u sil ve yeniden indir.",
    errExtractorTitle: "PkgExtractor çalışmıyor",
    errExtractorBody:
      "Çıkartma aracı eksik ya da başlatılamadı. Ayarlar’dan extractor’ı sil; gerekiyorsa kurulum yeniden indirir (~1 MB).",
    errGenericTitle: "Kurulum hatası",
    errGenericBody:
      "Beklenmeyen bir hata oldu. Mesajı not et, gerekirse Ayarlar’dan temizleyip tekrar dene. Devam etmek için aşağıdaki oynat tuşuna bas.",
    portSettings: "Oyun & Grafik",
    portSettingsSub:
      "bbport ayarları. Oyun ve kayıt klasörleri, FPS, ölçekleyici, ön ayar ve çıkış çözünürlüğü. Kaydet yazınca hemen uygulanır.",
    gameLocation: "Oyun konumu",
    gameLocationSub: "program/konumu\\game\\CUSA03173 — CUSA03173 dump klasörü",
    saveLocation: "Kayıt konumu",
    saveLocationSub: "Kayıtlar ve shader cache. Varsayılan: Belgeler\\Bloodborne\\saves",
    fpsLabel: "Kare hızı",
    upscalerLabel: "Ölçekleyici",
    presetLabel: "Ön ayar",
    outputLabel: "Çıkış",
    savePortBtn: "Ayarları Kaydet",
    portSaved: "Ayarlar kaydedildi",
    portSavedSub: "bbport bir sonraki açılışta kullanır",
    portSaveErr: "Ayarlar yazılamadı",
    browseFolder: "Klasör seç",
    fps_30: "30 FPS (PS4 gibi)",
    fps_60: "60 FPS",
    fps_90: "90 FPS",
    fps_uncap: "Sınırsız",
    preset_0: "Native AA (×1.0)",
    preset_1: "Quality (×1.5)",
    preset_2: "Balanced (×1.7)",
    preset_3: "Performance (×2)",
    preset_4: "Ultra Performance (×3)",
    readyPlayHint: "Kurulum hazır — av başlasın",
    readyConfig: "Yapılandırma",
    readyGamePath: "Oyun yolu",
    readySavePath: "Kayıt yolu",

    errTechLabel: "Teknik detay",
    errStepFailed: "Bu adımda durdu",
  },
  en: {
    minimize: "Minimize",
    trayHint: "Minimize to tray",
    quit: "Close",
    lang: "TR",

    install: "Install",
    prepTitle: "Ready your hunter <span>for the hunt</span>",
    prepSub:
      "Game PKG, update, PkgExtractor, and the Bloodborne PC port download in one flow. Required free space accounts for packages already on disk. Choose an install folder and continue.",
    rootFolder: "Install root folder",
    folderNotChosen: "No folder selected",
    change: "Change",
    startInstall: "Start Install",
    diskWaiting: "Waiting for disk info",
    freeSpace: "Free space",
    required: "Required",
    status: "Status",
    freeLabel: "free",
    enough: "Enough",
    notEnough: "Not enough",
    diskFull: "Free",
    ofNeeded: "/ needed",
    contentsNote:
      "Contents: base PKG (~29 GB), update PKG, Supermedo Windows port, and AzaharPlus PkgExtractor. Downloads resume where they left off.",

    flow: "Install flow",
    galleryDetail: "Download details",
    gallerySettings: "Settings",
    galleryPath: "Change folder",
    openDetailHint: "Open detailed download page",

    pending: "Waiting",
    runningBadge: "Active",
    done: "Done",
    skipped: "Skipped",
    errorBadge: "Error",

    st_idle: "Starting",
    st_running: "Downloading",
    st_paused: "Paused",
    st_extracting: "Preparing files",
    st_ready: "Ready",
    st_error: "Error",

    step_download_base: "Downloading game PKG",
    step_download_update: "Downloading update PKG",
    step_download_port: "Downloading port",
    step_download_extractor: "Downloading PkgExtractor",
    step_extract_base: "Extracting game files",
    step_extract_update: "Extracting update",
    step_merge_update: "Applying update",
    step_extract_port: "Unpacking port files",
    step_write_settings: "Writing settings",

    filesUnit: "files",
    leftUnit: "left",

    file_base: "Bloodborne GOTY PKG",
    file_update: "CUSA03173 Update v1.09 PKG",
    file_port: "Bloodborne-Windows.zip",
    file_extractor: "PkgExtractor 1.1",
    file_default: "Downloading",

    nextUp: "Next:",

    detailEyebrow: "Download details",
    detailSub: "Step-by-step progress. Use the mini panel to return to the gallery.",
    galleryBack: "Gallery",
    ready: "Ready",
    pause: "Pause",
    resume: "Resume",
    pathChange: "Change path",
    settingsBtn: "Settings",
    sourceNote: "Sources: archive.org PKGs + Supermedo bloodborne_pc",

    readyKicker: "CUSA03173 v1.09",
    readySub: "Install complete. Hunter's Dream awaits.",
    readyStatus: "Status",
    readyOrb: "Game files are ready. Port and settings are in place.",
    play: "PLAY",
    stop: "STOP",
    advancedBtn: "Advanced",
    advancedHint: "Settings / cheats / achievements / online panel",
    advancedStarted: "Bloodborne.exe launched",
    advancedErr: "Could not start Bloodborne.exe",
    cleanTempBtn: "Delete temporary files",
    cleanTempOnlyReady: "Temporary files can only be deleted after a full install",
    gameStopped: "Game stopped",
    gameStoppedSub: "Play Bloodborne.exe and Bloodborne.exe closed",
    recheck: "Recheck",

    settingsTitle: "Storage & cleanup",
    settingsSub:
      "Choose what to delete. Safe options only remove leftovers; red options require a reinstall.",
    safeClean: "Safe cleanup",
    safeCleanSub: "Won't break the game. No re-download needed.",
    partialDl: "Partial downloads (.part)",
    partialDlSub: "Leftovers from interrupted downloads. Next install starts clean.",
    patchTmp: "Update temp folder",
    patchTmpSub: "game/CUSA03173-patch — leftover after update is applied.",
    extractorTool: "Remove PkgExtractor",
    extractorToolSub: "tools/ folder. Re-downloads on update step if needed (~1 MB).",
    resetSec: "Reset install",
    resetSecSub: "If these files are deleted you will need to download again.",
    allPkgs: "All PKG and zip files",
    allPkgsSub: "~29 GB game PKG + update + port + extractor. Re-download required.",
    extractedGame: "Extracted game and port",
    extractedGameSub: "game/ and port/ are deleted. Install starts over (downloads stop).",
    installPath: "Install path",
    noPath: "none",
    settingsPathNote:
      "Cleanup applies to this folder. Use “Change path” on the main screen to change it.",
    cleanBtn: "Clean selected",
    back: "Back",

    confirmAllPkgs: "All PKG and zip files will be deleted. Are you sure?",
    confirmExtracted: "Extracted game and port will be deleted. Are you sure?",
    cleanDone: "Cleanup finished",
    cleanDoneSub: "files/folders removed",
    cleanErr: "Cleanup error",

    folderPickTitle: "Select install folder",
    folderUpdated: "Folder updated",
    folderErr: "Could not read folder",
    diskErr: "Could not read disk info",
    noDisk: "Not enough space",
    noDiskSub: "At least 65 GB free is required",
    installStartErr: "Could not start install",
    checkErr: "Check failed",
    stateErr: "Could not read state",
    bootErr: "Startup error",
    errUnknown: "Unknown error",
    errTitle: "Error",
    installDone: "Install complete",
    installDoneSub: "PLAY button is ready",
    readyToast: "Ready",
    readyToastSub: "Bloodborne is ready to play",
    gameStarted: "Bloodborne launched",
    gameStartedSub: "Options appear after the game exits",
    gameOpenErr: "Could not start game",
    shutdownTitle: "Game closed",
    shutdownQ: "What do you want to do now?",
    stayLauncher: "Stay in launcher",
    shutdownPc: "Shut down PC",
    restartPc: "Restart",
    shuttingDown: "Shutting down",
    shuttingDownSub: "PC will power off in 60 seconds",
    shutdownErr: "Shutdown error",
    restarting: "Restarting",
    restartingSub: "PC will restart in 60 seconds",
    restartErr: "Restart error",

    sn: "s",
    min: "m",
    hour: "h",
    almost: "almost done",

    errDiskTitle: "Not enough disk space",
    errDiskBody:
      "At least 65 GB free space is required. Free up space or choose another drive, then try again.",
    errExtractTitle: "Could not extract files",
    errExtractBody:
      "PkgExtractor failed to finish. The archive may be corrupt or the folder is write-protected. Clear extracted files in Settings and restart the install.",
    errEbootTitle: "eboot.bin not found",
    errEbootBody:
      "Extraction finished but the required eboot.bin is missing. The PKG may be incomplete. Clear the game folder in Settings and retry.",
    errDownloadTitle: "Download failed",
    errDownloadBody:
      "Could not download a PKG or the port. Check your connection; partial files resume where they left off.",
    errZipTitle: "Archive could not open",
    errZipBody:
      "The port/zip file is unreadable or corrupt. Delete the port in Settings and download again.",
    errExtractorTitle: "PkgExtractor failed",
    errExtractorBody:
      "The extractor tool is missing or could not start. Remove it in Settings; it re-downloads if needed (~1 MB).",
    errGenericTitle: "Install error",
    errGenericBody:
      "Something unexpected happened. Note the message, clean up in Settings if needed, and retry. Press resume below to continue.",
    portSettings: "Game & Graphics",
    portSettingsSub:
      "bbport settings. Game and save folders, FPS, upscaler, preset and output resolution. Saved values apply on the next launch.",
    gameLocation: "Game location",
    gameLocationSub: "install/root\\game\\CUSA03173 — the CUSA03173 dump folder",
    saveLocation: "Save location",
    saveLocationSub: "Saves and shader cache. Default: Documents\\Bloodborne\\saves",
    fpsLabel: "Frame rate",
    upscalerLabel: "Upscaler",
    presetLabel: "Preset",
    outputLabel: "Output",
    savePortBtn: "Save settings",
    portSaved: "Settings saved",
    portSavedSub: "bbport will use them on the next launch",
    portSaveErr: "Could not write settings",
    browseFolder: "Choose folder",
    fps_30: "30 FPS (like PS4)",
    fps_60: "60 FPS",
    fps_90: "90 FPS",
    fps_uncap: "Unlocked",
    preset_0: "Native AA (×1.0)",
    preset_1: "Quality (×1.5)",
    preset_2: "Balanced (×1.7)",
    preset_3: "Performance (×2)",
    preset_4: "Ultra Performance (×3)",
    readyPlayHint: "Install complete — the hunt begins",
    readyConfig: "Configuration",
    readyGamePath: "Game path",
    readySavePath: "Save path",

    errTechLabel: "Technical detail",
    errStepFailed: "Stopped on this step",
  },
};

function detect() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "tr" || saved === "en") return saved;
  } catch {
    /* ignore */
  }
  const nav = (navigator.language || "tr").toLowerCase();
  return nav.startsWith("tr") ? "tr" : "en";
}

let lang = detect();

export function getLang() {
  return lang;
}

export function setLang(next) {
  if (next !== "tr" && next !== "en") return;
  lang = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    /* ignore */
  }
}

export function toggleLang() {
  setLang(lang === "tr" ? "en" : "tr");
}

export function t(key) {
  const row = dict[lang] || dict.tr;
  return row[key] ?? dict.tr[key] ?? key;
}

export function tStep(stepId) {
  return t(`step_${stepId}`);
}

export function tStatus(status) {
  const map = {
    idle: "st_idle",
    running: "st_running",
    paused: "st_paused",
    extracting: "st_extracting",
    ready: "st_ready",
    error: "st_error",
  };
  return t(map[status] || "st_idle");
}

export function tBadge(status) {
  const map = {
    pending: "pending",
    running: "runningBadge",
    done: "done",
    skipped: "skipped",
    error: "errorBadge",
  };
  return t(map[status] || "pending");
}

export function tFile(stepId) {
  const map = {
    download_base: "file_base",
    download_update: "file_update",
    download_port: "file_port",
    download_extractor: "file_extractor",
  };
  return t(map[stepId] || "file_default");
}
