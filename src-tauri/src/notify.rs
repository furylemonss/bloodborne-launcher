use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn installed(app: &AppHandle) {
    let _ = app
        .notification()
        .builder()
        .title("Kurulum tamamlandı")
        .body("Bloodborne hazır. OYNA butonuna basabilirsin.")
        .show();
}

pub fn game_exited(app: &AppHandle) {
    let _ = app
        .notification()
        .builder()
        .title("Oyun kapandi")
        .body("Kapatmak, kapatıcı veya yeniden başlatmak için pencereye dön.")
        .show();
}
