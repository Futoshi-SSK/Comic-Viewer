use base64::{engine::general_purpose, Engine as _};
use std::collections::HashMap;
use std::io::{Read, Seek};
use tauri::{Emitter, Listener, Manager};

#[derive(serde::Serialize)]
struct PageData {
    data: String,
    mime: String,
}

fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
}

fn is_zip(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".cbz")
}

fn get_mime(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/jpeg"
    }
}

/// ZIPアーカイブを再帰的に探索して画像の仮想パスを収集する。
/// ネストされたZIPは prefix::inner.zip::image.jpg 形式でパスを生成する。
fn collect_images_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    prefix: &str,
    results: &mut Vec<String>,
) {
    // まずエントリ名を全収集（borrowを解放するため）
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            archive
                .by_index(i)
                .ok()
                .and_then(|e| if !e.is_dir() { Some(e.name().to_string()) } else { None })
        })
        .collect();

    let mut nested: Vec<(String, Vec<u8>)> = Vec::new();

    for name in &names {
        let full_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", prefix, name)
        };

        if is_image(name) {
            results.push(full_path);
        } else if is_zip(name) {
            // ネストされたZIPのデータを読み込む
            let mut data = Vec::new();
            if let Ok(mut entry) = archive.by_name(name) {
                let _ = entry.read_to_end(&mut data);
            }
            if !data.is_empty() {
                nested.push((full_path, data));
            }
        }
    }

    // ネストされたZIPを再帰処理
    for (zip_path, data) in nested {
        let cursor = std::io::Cursor::new(data);
        if let Ok(mut inner) = zip::ZipArchive::new(cursor) {
            collect_images_from_archive(&mut inner, &zip_path, results);
        }
    }
}

/// 仮想パス（"::"区切り）に従ってアーカイブを再帰的に読み込む。
fn read_image_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    parts: &[&str],
) -> Result<PageData, String> {
    if parts.is_empty() {
        return Err("パスが空です".to_string());
    }

    if parts.len() == 1 {
        // 目的の画像ファイル
        let name = parts[0];
        let mut data = Vec::new();
        {
            let mut entry = archive.by_name(name).map_err(|e| e.to_string())?;
            entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        }
        Ok(PageData {
            data: general_purpose::STANDARD.encode(&data),
            mime: get_mime(name).to_string(),
        })
    } else {
        // ネストされたZIPを開いて再帰
        let zip_name = parts[0];
        let mut zip_data = Vec::new();
        {
            let mut entry = archive.by_name(zip_name).map_err(|e| e.to_string())?;
            entry.read_to_end(&mut zip_data).map_err(|e| e.to_string())?;
        }
        let cursor = std::io::Cursor::new(zip_data);
        let mut inner = zip::ZipArchive::new(cursor).map_err(|e| e.to_string())?;
        read_image_from_archive(&mut inner, &parts[1..])
    }
}

/// ZIPを開いて画像の仮想パスリストを返す（多重圧縮対応）。
#[tauri::command]
fn open_zip(path: String) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    collect_images_from_archive(&mut archive, "", &mut results);
    results.sort_by(|a, b| natord::compare(a, b));
    Ok(results)
}

/// 仮想パスを指定して画像データを返す。
#[tauri::command]
fn get_zip_page(path: String, virtual_path: String) -> Result<PageData, String> {
    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = virtual_path.split("::").collect();
    read_image_from_archive(&mut archive, &parts)
}

/// PDFファイルをbase64で返す。
#[tauri::command]
fn get_file_base64(path: String) -> Result<String, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(&data))
}

/// 閲覧位置を保存する。
/// file_path をキー、position（ZIPは仮想パス文字列、PDFは0-indexed番号の文字列）を値とする。
/// 保存先: %APPDATA%\comic-viewer\positions.json
#[tauri::command]
fn save_position(app: tauri::AppHandle, file_path: String, position: String) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json_path = dir.join("positions.json");

    let mut map: HashMap<String, String> = if json_path.exists() {
        let text = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        HashMap::new()
    };

    map.insert(file_path, position);
    let text = serde_json::to_string(&map).map_err(|e| e.to_string())?;
    std::fs::write(&json_path, text).map_err(|e| e.to_string())?;
    Ok(())
}

/// 閲覧位置を読み込む。保存がなければ空文字列を返す。
#[tauri::command]
fn load_position(app: tauri::AppHandle, file_path: String) -> Result<String, String> {
    let json_path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("positions.json");

    if !json_path.exists() {
        return Ok(String::new());
    }
    let text = std::fs::read_to_string(&json_path).map_err(|e| e.to_string())?;
    let map: HashMap<String, String> = serde_json::from_str(&text).unwrap_or_default();
    Ok(map.get(&file_path).cloned().unwrap_or_default())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 起動引数の1番目をファイルパスとして取得（"--"で始まるフラグは除外）
    let initial_file = std::env::args()
        .nth(1)
        .filter(|a| !a.starts_with("--"));

    tauri::Builder::default()
        .setup(move |app| {
            // フロントエンドの準備完了通知を受けたらファイルパスを送信
            if let Some(path) = initial_file {
                let handle = app.handle().clone();
                app.once("frontend-ready", move |_| {
                    handle.emit("open-file", path).ok();
                });
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_zip,
            get_zip_page,
            get_file_base64,
            save_position,
            load_position
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
