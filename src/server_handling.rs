use std::collections::{HashMap, VecDeque};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_deque::Worker;
use smol::block_on;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio::{sync::RwLock, task::JoinHandle};

use crate::config::{Config, CONFIG_PATH};
use crate::constants::{self, ConnectionTypes, DownloadStatus, Percentage};
use crate::events::SLSKEvents;
use crate::messages::{
    CantConnectToPeer, ConnectToPeer, FileSearch, GetPeerAddress, JoinRoom, LeaveRoom, Login,
    MessageTrait, RoomList, SayChatroom, SetWaitPort, SharedFileListRequest, SharedFoldersFiles,
    UserStats, _ReceiveConnectToPeer, _SendFileSearch, _SendGetPeerAddress, _SendJoinRoom,
    _SendLeaveRoom, _SendLogin, _SendRoomList, _SendSayChatroom,
};
use crate::utils::get_code_and_bytes_from_readable;
use crate::{messages::MessageType, SLSKExitCode};

/// Reads messages from the server and acts accordingly
pub(crate) async fn start_server_read_task(
    quit: Arc<RwLock<bool>>,
    logged_in: Arc<RwLock<bool>>,
    mut reader: OwnedReadHalf,
    write_queue: Sender<SLSKEvents>,
    indirect_peers_list_writer: Worker<_ReceiveConnectToPeer>,
    server_username: Arc<RwLock<Option<String>>>,
    config_username: String,
    user_info_map: Arc<Mutex<HashMap<String, (Ipv4Addr, u32)>>>,
) -> JoinHandle<SLSKExitCode> {
    tokio::spawn(async move {
        loop {
            if *quit.read().await {
                return SLSKExitCode::Ok;
            };
            let (code, mut bytes) =
                match get_code_and_bytes_from_readable(&mut reader, MessageType::Server(0)).await {
                    Ok((code, bytes)) => (code, bytes),
                    Err(e) => return e,
                };

            match code {
                MessageType::Server(1) => {
                    if let Some(response) = Login::from_stream(&mut bytes) {
                        // if we autologin and succeed, we don't receive a TryLogin event, so the username isn't set
                        // in this case, we know that the username is the config username
                        if server_username.read().await.is_none() {
                            *server_username.write().await = Some(config_username.clone());
                        }
                        let _ = write_queue.send(SLSKEvents::LoginResult {
                            success: response.success,
                            reason: response.failure_reason,
                        });
                        if !response.success {
                            drop(reader);
                            return SLSKExitCode::LoginFail;
                        }
                        *logged_in.write().await = true;
                    }
                }
                MessageType::Server(3) => {
                    if let Some(response) = GetPeerAddress::from_stream(&mut bytes) {
                        user_info_map
                            .lock()
                            .await
                            .insert(response.username.clone(), (response.ip, response.port));
                    }
                    // println!("{:#?}", GetPeerAddress::from_stream(&mut bytes));
                }
                MessageType::Server(5) => {
                    // println!("{:#?}", WatchUser::from_stream(&mut bytes));
                }
                MessageType::Server(7) => {
                    // println!("{:#?}", GetUserStatus::from_stream(&mut bytes));
                }
                MessageType::Server(13) => {
                    if let Some(response) = SayChatroom::from_stream(&mut bytes) {
                        let _ = write_queue.send(SLSKEvents::ChatroomMessage {
                            room: response.room,
                            username: Some(response.username),
                            message: response.message,
                        });
                    }
                }
                MessageType::Server(14) => {
                    if let Some(response) = JoinRoom::from_stream(&mut bytes) {
                        let _ = write_queue.send(SLSKEvents::UpdateRoom {
                            room: response.room,
                            stats: response
                                .usernames
                                .into_iter()
                                .zip(response.stats)
                                .collect::<Vec<(String, UserStats)>>(),
                        });
                    }
                }
                MessageType::Server(15) => {
                    // println!("{:#?}", LeaveRoom::from_stream(&mut bytes));
                }
                MessageType::Server(16) => {
                    // println!("{:#?}", UserJoinedRoom::from_stream(&mut bytes));
                }
                MessageType::Server(17) => {
                    // println!("{:#?}", UserLefRoom::from_stream(&mut bytes));
                }
                MessageType::Server(18) => {
                    // Ideally, this shouldn't happen if we receive the PeerInit message
                    // While still testing stuff out it's not the end of the world
                    // TODO: Handle peer connections appropriately depending on if we have an open port
                    if let Some(connect_request) = ConnectToPeer::from_stream(&mut bytes) {
                        match connect_request.connection_type {
                            constants::ConnectionTypes::PeerToPeer => {
                                let _ = indirect_peers_list_writer.push(connect_request);
                            }
                            constants::ConnectionTypes::FileTransfer => {
                                let _ = indirect_peers_list_writer.push(connect_request);
                            }
                            constants::ConnectionTypes::DistributedNetwork => (),
                        }
                    }
                }
                MessageType::Server(22) => {
                    // println!("{:#?}", MessageUser::from_stream(&mut bytes));
                }
                MessageType::Server(26) => {
                    // println!("{:#?}", FileSearch::from_stream(&mut bytes));
                }
                MessageType::Server(36) => {
                    // println!("{:#?}", GetUserStats::from_stream(&mut bytes));
                }
                MessageType::Server(41) => {
                    // println!("{:#?}", Relogged::from_stream(&mut bytes));
                    return SLSKExitCode::OtherLogin;
                }
                MessageType::Server(64) => {
                    if let Some(room_list) = RoomList::from_stream(&mut bytes) {
                        let rooms_and_num_of_users = room_list
                            .rooms
                            .into_iter()
                            .zip(room_list.num_of_users)
                            .collect();
                        let _ = write_queue.send(SLSKEvents::RoomList {
                            rooms_and_num_of_users,
                        });
                    }
                }
                MessageType::Server(66) => {
                    // println!("{:#?}", AdminMessage::from_stream(&mut bytes));
                }
                MessageType::Server(69) => {
                    // println!("{:#?}", PrivilegedUsers::from_stream(&mut bytes));
                }
                MessageType::Server(83) => {
                    // println!("{:#?}", ParentMinSpeed::from_stream(&mut bytes));
                }
                MessageType::Server(84) => {
                    // println!("{:#?}", ParentSpeedRatio::from_stream(&mut bytes));
                }
                MessageType::Server(92) => {
                    // println!("{:#?}", CheckPrivileges::from_stream(&mut bytes));
                }
                MessageType::Server(93) => {
                    // println!("{:#?}", EmbeddedMessage::from_stream(&mut bytes));
                }
                MessageType::Server(102) => {
                    // println!("{:#?}", PossibleParents::from_stream(&mut bytes));
                }
                MessageType::Server(104) => {
                    // println!("{:#?}", WishListInterval::from_stream(&mut bytes));
                }
                MessageType::Server(113) => {
                    // println!("{:#?}", RoomTickerState::from_stream(&mut bytes));
                }
                MessageType::Server(114) => {
                    // println!("{:#?}", RoomTickerAdd::from_stream(&mut bytes));
                }
                MessageType::Server(115) => {
                    // println!("{:#?}", RoomTickerRemove::from_stream(&mut bytes));
                }
                MessageType::Server(130) => {
                    // println!("{:#?}", ResetDistributed::from_stream(&mut bytes));
                }
                MessageType::Server(133) => {
                    // println!("{:#?}", PrivateRoomUsers::from_stream(&mut bytes));
                }
                MessageType::Server(134) => {
                    // println!("{:#?}", PrivateRoomAddUser::from_stream(&mut bytes));
                }
                MessageType::Server(135) => {
                    // println!("{:#?}", PrivateRoomRemoveUser::from_stream(&mut bytes));
                }
                MessageType::Server(139) => {
                    // println!("{:#?}", PrivateRoomAdded::from_stream(&mut bytes));
                }
                MessageType::Server(140) => {
                    // println!("{:#?}", PrivateRoomRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(141) => {
                    // println!("{:#?}", PrivateRoomToggle::from_stream(&mut bytes));
                }
                MessageType::Server(142) => {
                    // println!("{:#?}", ChangePassword::from_stream(&mut bytes));
                }
                MessageType::Server(143) => {
                    // println!("{:#?}", PrivateRoomAddOperator::from_stream(&mut bytes));
                }
                MessageType::Server(144) => {
                    // println!("{:#?}", PrivateRoomRemoveOperator::from_stream(&mut bytes));
                }
                MessageType::Server(145) => {
                    // println!("{:#?}", PrivateRoomOperatorAdded::from_stream(&mut bytes));
                }
                MessageType::Server(146) => {
                    // println!("{:#?}", PrivateRoomOperatorRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(148) => {
                    // println!("{:#?}", PrivateRoomOwned::from_stream(&mut bytes));
                }
                MessageType::Server(1001) => {
                    println!("cant connect to peer {bytes:?} ");
                    print!("{:#?}", CantConnectToPeer::from_stream(&mut bytes));
                }
                MessageType::Server(1003) => {
                    // println!("{:#?}", CantConnectToRoom::from_stream(&mut bytes));
                }
                _ => (),
            }
        }
    })
}

/// Receives events and writes to the server accordingly
pub(crate) async fn start_server_write_task(
    quit_write: Arc<RwLock<bool>>,
    config: Arc<RwLock<Config>>,
    mut read_queue: Receiver<SLSKEvents>,
    mut writer: OwnedWriteHalf,
    my_username: Arc<RwLock<Option<String>>>,
    my_port: u32,
    token_message_map: Arc<Mutex<HashMap<u32, VecDeque<Vec<u8>>>>>,
    writer_user_info_map: Arc<Mutex<HashMap<String, (Ipv4Addr, u32)>>>,
    writer_write_queue: Sender<SLSKEvents>,
    prompted_peers_list_writer: Worker<(String, u32, ConnectionTypes)>,
    download_filename_map: Arc<
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
) -> JoinHandle<SLSKExitCode> {
    tokio::spawn({
        let my_username = Arc::clone(&my_username);
        async move {
            {
                let config = config.read().await;
                let username = &config.user.name;
                let password = &config.user.password;
                if config.server.auto_connect & !username.is_empty() & !password.is_empty() {
                    let login_info = _SendLogin::new(username.to_owned(), password.to_owned());
                    let _ = block_on(Login::async_write_to(&mut writer, login_info).await);
                }
            };
            loop {
                sleep(Duration::from_nanos(1)).await;
                let event = read_queue.recv().await;
                match event {
                    Ok(event) => match event {
                        SLSKEvents::TryLogin { username, password } => {
                            *my_username.write().await = Some(username.clone());
                            {
                                let mut locked_config = config.write().await;
                                locked_config.user.name = username.clone();
                                locked_config.user.password = password.clone();
                                locked_config.write_to_file(Path::new(CONFIG_PATH), true);
                            }
                            let login_info = _SendLogin::new(username, password);
                            let _ = block_on(Login::async_write_to(&mut writer, login_info).await);
                        }
                        SLSKEvents::Quit { restart } => {
                            *quit_write.write().await = true;
                            let _ = writer.shutdown();
                            return if restart {
                                SLSKExitCode::Ok
                            } else {
                                SLSKExitCode::Restart
                            };
                        }
                        SLSKEvents::LoginResult { success, .. } => {
                            if success {
                                let _ = block_on(
                                    SetWaitPort::async_write_to(
                                        &mut writer,
                                        SetWaitPort {
                                            port: my_port,
                                            _unknown: 0,
                                            obfuscated_port: my_port, // TODO: Implement SetWaitPort obfuscated_port
                                        },
                                    )
                                    .await,
                                );

                                let shared_folders_and_files = {
                                    let index = &config.read().await.index;
                                    SharedFoldersFiles {
                                        dirs: index.get_folder_count().await.unwrap_or_default(),
                                        files: index
                                            .get_total_file_count()
                                            .await
                                            .unwrap_or_default(),
                                    }
                                };
                                let _ = block_on(
                                    SharedFoldersFiles::async_write_to(
                                        &mut writer,
                                        shared_folders_and_files,
                                    )
                                    .await,
                                );

                                let _ = block_on(
                                    RoomList::async_write_to(&mut writer, _SendRoomList {}).await,
                                );
                            }
                        }
                        SLSKEvents::RoomList { .. } => (),
                        SLSKEvents::JoinRoom { room, private } => {
                            let _ = block_on(
                                JoinRoom::async_write_to(
                                    &mut writer,
                                    _SendJoinRoom { room, private },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::LeaveRoom { room } => {
                            let _ = block_on(
                                LeaveRoom::async_write_to(&mut writer, _SendLeaveRoom { room })
                                    .await,
                            );
                        }
                        SLSKEvents::UpdateRoom { .. } => (),
                        SLSKEvents::ChatroomMessage {
                            room,
                            username,
                            message,
                        } => match username {
                            Some(_) => (),
                            None => {
                                let _ = block_on(
                                    SayChatroom::async_write_to(
                                        &mut writer,
                                        _SendSayChatroom { room, message },
                                    )
                                    .await,
                                );
                            }
                        },
                        SLSKEvents::SearchResults { .. } => (),
                        SLSKEvents::FileSearch { query, token } => {
                            let _ = block_on(
                                FileSearch::async_write_to(
                                    &mut writer,
                                    _SendFileSearch {
                                        token,
                                        search_query: query,
                                    },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::QueueMessage {
                            token,
                            message_bytes,
                        } => {
                            token_message_map
                                .lock()
                                .await
                                .entry(token)
                                .or_default()
                                .push_back(message_bytes);
                        }
                        SLSKEvents::Connect {
                            username,
                            token,
                            connection_type,
                        } => {
                            if !writer_user_info_map.lock().await.contains_key(&username) {
                                writer_write_queue
                                    .send(SLSKEvents::GetInfo(username.clone()))
                                    .unwrap();
                            };
                            prompted_peers_list_writer.push((username, token, connection_type));
                        }
                        SLSKEvents::GetInfo(username) => {
                            let _ = block_on(
                                GetPeerAddress::async_write_to(
                                    &mut writer,
                                    _SendGetPeerAddress { username },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::NewDownloads { .. } => (),
                        SLSKEvents::NewDownload { .. } => (),
                        SLSKEvents::UpdateDownload {
                            filename,
                            status,
                            percentage,
                        } => {
                            download_filename_map
                                .lock()
                                .await
                                .entry(filename)
                                .or_default()
                                .push_back((status, percentage, None));
                        }
                        SLSKEvents::UpdateDownloads { files, from_all } => {
                            let mut download_filename_map = download_filename_map.lock().await;
                            for (filename, status, percentage) in files {
                                download_filename_map
                                    .entry(filename)
                                    .or_default()
                                    .push_back((status, percentage, Some(from_all)));
                            }
                        }
                        SLSKEvents::BrowseUser { username } => {
                            let token = rand::random();
                            writer_write_queue
                                .send(SLSKEvents::QueueMessage {
                                    token,
                                    message_bytes: SharedFileListRequest::to_bytes(
                                        SharedFileListRequest {},
                                    ),
                                })
                                .unwrap();
                            writer_write_queue
                                .send(SLSKEvents::Connect {
                                    username,
                                    token,
                                    connection_type: ConnectionTypes::PeerToPeer,
                                })
                                .unwrap();
                        }
                    },
                    Err(_) => {
                        *quit_write.write().await = true;
                        let _ = writer.shutdown();
                        return SLSKExitCode::Ok;
                    }
                };
            }
        }
    })
}
