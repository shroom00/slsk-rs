use std::{
    collections::{HashMap, VecDeque},
    fs::{create_dir, create_dir_all},
    io::Write,
    path::Path,
    sync::Arc,
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{Mutex, RwLock},
    time::sleep,
};

use crate::{
    constants::{DownloadStatus, Percentage},
    utils::log,
    CHUNK_SIZE,
};

// TODO: Implement uploads
/// Handles downloading a file, will handle uploading files in the future
pub(crate) async fn handle_file_transfer(
    mut peer_stream: TcpStream,
    file_info_map: Arc<Mutex<HashMap<u32, VecDeque<(String, u64)>>>>,
    peer_download_filename_map: Arc<
        Mutex<
            HashMap<
                String,
                VecDeque<(
                    Arc<RwLock<DownloadStatus>>,
                    Arc<RwLock<Percentage>>,
                    Option<bool>,
                )>,
            >,
        >,
    >,
    username: String,
) {
    let offset = 0;
    let mut percentage = 0u8;
    let mut downloaded = offset;
    let file_init_token = peer_stream.read_u32_le().await.unwrap();
    let (filename, filesize) = {
        file_info_map
            .lock()
            .await
            .get_mut(&file_init_token)
            .unwrap()
            .pop_front()
            .unwrap()
    };
    let (download_status, download_percentage, download_type) = {
        loop {
            {
                let mut locked_peer_download_filename_map = peer_download_filename_map.lock().await;
                if let Some(all_download_info) =
                    locked_peer_download_filename_map.get_mut(&filename)
                {
                    if let Some(download_info) = all_download_info.pop_front() {
                        if all_download_info.is_empty() {
                            locked_peer_download_filename_map.remove(&filename);
                        }
                        break download_info;
                    }
                };
                sleep(Duration::from_millis(500)).await;
            }
        }
    };
    {
        *download_status.write().await = DownloadStatus::Starting;
    }
    let mut file_handle = std::fs::File::create({
        let (prefix, base_name) = filename.rsplit_once("\\").unwrap();
        let filepath = match download_type {
            Some(is_all) => {
                let folder = prefix.rsplit_once("\\").unwrap().1;

                {
                    if is_all {
                        let folder_path = Path::new(&username).join(folder);
                        if !std::fs::exists(&username).unwrap() {
                            create_dir_all(&folder_path).unwrap();
                        } else if !std::fs::exists(&folder_path).unwrap() {
                            create_dir(&folder_path).unwrap();
                        };
                        folder_path
                    } else {
                        let folder_path = Path::new(&folder);
                        if !std::fs::exists(&folder_path).unwrap() {
                            create_dir(&folder_path).unwrap();
                        };
                        folder_path.to_path_buf()
                    }
                }
                .join(base_name)
            }
            None => base_name.into(),
        };
        if filepath.exists() {
            let mut count = 1;
            let base_name = filepath.with_extension("");
            let base_name = base_name.to_string_lossy();
            let extension = filepath
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();

            loop {
                let new_filepath =
                    Path::new(&format!("{base_name} ({count}){extension}",)).to_path_buf();
                if new_filepath.exists() {
                    count += 1;
                } else {
                    break new_filepath;
                }
            }
        } else {
            filepath
        }
    })
    .unwrap();
    peer_stream.write_u64_le(offset).await.unwrap();

    loop {
        sleep(Duration::from_nanos(1)).await;
        let mut buf = vec![0; std::cmp::min((filesize - downloaded) as usize, CHUNK_SIZE,)];
        {
            *download_status.write().await = DownloadStatus::Downloading;
        }
        match peer_stream.read_exact(&mut buf).await {
            Ok(n) => {
                downloaded += n as u64;
                file_handle.write_all(&buf).unwrap();
                if downloaded == filesize {
                    log(format!("finished downloading {file_handle:?}"));
                    {
                        *download_status.write().await = DownloadStatus::Complete;

                        *download_percentage.write().await = Percentage(100);
                    }
                    break;
                }
                let new_percentge = ((downloaded * 100) / filesize) as u8;
                if new_percentge != percentage {
                    {
                        *download_percentage.write().await = Percentage(new_percentge);
                    }
                    percentage = new_percentge;
                }
            }
            Err(e) => {
                log(format!("stopped downloading {file_handle:?} due to {e:?}"));
                *download_status.write().await = DownloadStatus::Failed;
                break;
            }
        }
    }
    file_handle.flush().unwrap();
    let _ = peer_stream.shutdown().await;
    return;
}
