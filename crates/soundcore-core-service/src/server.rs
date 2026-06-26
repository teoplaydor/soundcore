//! IPC server: accepts UI connections on the named pipe, dispatches each
//! incoming [`Request`] to the appropriate service, sends a [`Response`].

use anyhow::Context;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use soundcore_ipc::proto;
use soundcore_ipc::proto::{frame::Body as FrameBody, response::Payload as ResponsePayload};
use soundcore_ipc::transport::Listener;
use soundcore_ipc::PIPE_NAME;

use crate::audio_ops;
use crate::runtime::Services;

pub async fn serve(services: Arc<Services>, cancel: CancellationToken) -> anyhow::Result<()> {
    let mut listener = Listener::bind(PIPE_NAME).context("bind named pipe")?;
    info!(pipe = PIPE_NAME, "IPC listening");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("server: cancellation observed");
                return Ok(());
            }
            accept = listener.accept() => {
                let conn = match accept {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = ?e, "listener.accept() failed; retrying");
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        continue;
                    }
                };
                let services = services.clone();
                let cancel = cancel.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(conn, services, cancel).await {
                        warn!(error = ?e, "connection ended with error");
                    }
                });
            }
        }
    }
}

async fn handle_connection(
    mut conn: soundcore_ipc::transport::ServerConnection,
    services: Arc<Services>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    debug!("UI connection opened");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            next = conn.next() => {
                let Some(frame) = next else { return Ok(()); };
                let frame = frame.context("read frame")?;
                let Some(FrameBody::Request(req)) = frame.body else {
                    // Clients shouldn't send Responses or Events.
                    continue;
                };
                let response = dispatch(&services, req).await;
                let out = proto::Frame {
                    request_id: frame.request_id,
                    body: Some(FrameBody::Response(response)),
                };
                conn.send(out).await.context("send response")?;
            }
        }
    }
}

async fn dispatch(services: &Services, req: proto::Request) -> proto::Response {
    use proto::request::Payload as P;

    let payload = match req.payload {
        Some(p) => p,
        None => {
            return error_response(
                proto::ErrorCode::InvalidArg,
                "request payload missing".into(),
            );
        }
    };

    match payload {
        P::ListRenderDevices(_) => devices_response(soundcore_audio::DataFlow::Render).await,
        P::ListCaptureDevices(_) => devices_response(soundcore_audio::DataFlow::Capture).await,
        P::ListProcesses(_) => processes_response().await,
        P::ListCameras(_) => cameras_response().await,
        P::ListVstPlugins(_) | P::RescanVstPlugins(_) => vst_response().await,
        P::SetDeviceChain(_) | P::ClearDeviceChain(_)
        | P::SetProcessChain(_) | P::ClearProcessChain(_) => {
            error_response(proto::ErrorCode::NotReady, "FX engine not yet wired".into())
        }
        P::ListBindings(_) => ok_with(ResponsePayload::BindingList(proto::BindingList::default())),
        P::SetCameraMultiplex(cfg) => set_camera_response(services, cfg),
        P::GetCameraMultiplex(_) => get_camera_response(services),
        P::SetMicLock(cfg) => set_mic_lock_response(services, cfg),
        P::GetMicLock(_) => get_mic_lock_response(services),
        P::SetVstParameter(_) => error_response(
            proto::ErrorCode::NotReady,
            "live VST parameter changes not yet wired".into(),
        ),
        P::Subscribe(_) | P::Unsubscribe(_) => ok_with(ResponsePayload::Ok(proto::Empty {})),
    }
}

fn ok_with(p: ResponsePayload) -> proto::Response {
    proto::Response { payload: Some(p) }
}

async fn devices_response(flow: soundcore_audio::DataFlow) -> proto::Response {
    match audio_ops::list_devices(flow).await {
        Ok(devices) => {
            let proto_devices = devices
                .into_iter()
                .map(|d| proto::AudioDevice {
                    id: Some(proto::DeviceId { value: d.id }),
                    friendly_name: d.friendly_name,
                    interface_name: d.interface_name,
                    flow: data_flow_to_proto(d.flow).into(),
                    is_default: d.is_default,
                    is_default_communications: d.is_default_communications,
                    sample_rate: d.sample_rate,
                    channel_count: d.channel_count,
                    bits_per_sample: d.bits_per_sample,
                    master_volume: d.master_volume,
                    mute: d.mute,
                    soundcore_apo_active: false,
                })
                .collect();
            ok_with(ResponsePayload::DeviceList(proto::DeviceList {
                devices: proto_devices,
            }))
        }
        Err(e) => {
            error!(error = ?e, "device enumeration failed");
            error_response(proto::ErrorCode::Internal, e.to_string())
        }
    }
}

async fn vst_response() -> proto::Response {
    match audio_ops::list_vst_plugins().await {
        Ok(plugins) => {
            let proto_plugins = plugins
                .into_iter()
                .map(|p| proto::VstPluginInfo {
                    uid: p.uid,
                    name: p.name,
                    vendor: p.vendor,
                    category: p.category,
                    path: p.path,
                    num_inputs: p.num_inputs,
                    num_outputs: p.num_outputs,
                    has_editor: p.has_editor,
                })
                .collect();
            ok_with(ResponsePayload::VstPluginList(proto::VstPluginList {
                plugins: proto_plugins,
            }))
        }
        Err(e) => {
            error!(error = ?e, "VST scan failed");
            error_response(proto::ErrorCode::VstLoadFail, e.to_string())
        }
    }
}

fn set_camera_response(
    services: &Services,
    cfg: proto::CameraMultiplexConfig,
) -> proto::Response {
    let pref = cfg.preferred_format.unwrap_or_default();
    let result = services.config.with_mut(|c| {
        c.camera = crate::config::CameraConfig {
            enabled: cfg.enabled,
            source_symbolic_link: cfg.source_symbolic_link.clone(),
            width: pref.width,
            height: pref.height,
            frame_rate_num: pref.frame_rate_num,
            frame_rate_den: pref.frame_rate_den,
        };
    });
    if let Err(e) = result {
        return error_response(proto::ErrorCode::Internal, e.to_string());
    }
    services.apply_config();
    get_camera_response(services)
}

fn get_camera_response(services: &Services) -> proto::Response {
    let c = services.config.snapshot().camera;
    ok_with(ResponsePayload::CameraMultiplex(proto::CameraMultiplexConfig {
        enabled: c.enabled,
        source_symbolic_link: c.source_symbolic_link,
        preferred_format: Some(proto::CameraFormat {
            width: c.width,
            height: c.height,
            frame_rate_num: c.frame_rate_num,
            frame_rate_den: c.frame_rate_den,
            subtype: String::new(),
        }),
    }))
}

fn set_mic_lock_response(
    services: &Services,
    cfg: proto::MicLockConfig,
) -> proto::Response {
    let device_id = cfg
        .device
        .as_ref()
        .map(|d| d.value.clone())
        .unwrap_or_default();
    if cfg.enabled && device_id.is_empty() {
        return error_response(
            proto::ErrorCode::InvalidArg,
            "mic-lock device id is required when enabling".into(),
        );
    }
    let result = services.config.with_mut(|c| {
        c.mic_lock = crate::config::MicLockConfig {
            enabled: cfg.enabled,
            device_id: device_id.clone(),
            locked_volume: if cfg.locked_volume < 0.0 {
                None
            } else {
                Some(cfg.locked_volume)
            },
            also_lock_mute: cfg.also_lock_mute,
            revert_immediately: cfg.revert_immediately,
            allowed_image_globs: cfg.allowed_image_globs.clone(),
        };
    });
    if let Err(e) = result {
        return error_response(proto::ErrorCode::Internal, e.to_string());
    }
    services.apply_config();
    get_mic_lock_response(services)
}

fn get_mic_lock_response(services: &Services) -> proto::Response {
    let c = services.config.snapshot().mic_lock;
    ok_with(ResponsePayload::MicLock(proto::MicLockConfig {
        enabled: c.enabled,
        device: if c.device_id.is_empty() {
            None
        } else {
            Some(proto::DeviceId { value: c.device_id })
        },
        locked_volume: c.locked_volume.unwrap_or(-1.0),
        also_lock_mute: c.also_lock_mute,
        revert_immediately: c.revert_immediately,
        allowed_image_globs: c.allowed_image_globs,
    }))
}

async fn cameras_response() -> proto::Response {
    match audio_ops::list_cameras().await {
        Ok(cameras) => {
            let proto_cameras = cameras
                .into_iter()
                .map(|c| proto::CameraSource {
                    symbolic_link: c.symbolic_link,
                    friendly_name: c.friendly_name,
                    supported_formats: c
                        .formats
                        .into_iter()
                        .map(|f| proto::CameraFormat {
                            width: f.width,
                            height: f.height,
                            frame_rate_num: f.frame_rate_num,
                            frame_rate_den: f.frame_rate_den,
                            subtype: f.subtype,
                        })
                        .collect(),
                })
                .collect();
            ok_with(ResponsePayload::CameraList(proto::CameraList {
                cameras: proto_cameras,
            }))
        }
        Err(e) => {
            error!(error = ?e, "camera enumeration failed");
            error_response(proto::ErrorCode::Internal, e.to_string())
        }
    }
}

async fn processes_response() -> proto::Response {
    match audio_ops::list_processes().await {
        Ok(processes) => {
            let proto_processes = processes
                .into_iter()
                .map(|p| proto::Process {
                    pid: p.pid,
                    image_name: p.image_name,
                    image_path: p.image_path,
                    display_name: String::new(),
                    icon_png: prost::bytes::Bytes::new(),
                    sessions: Vec::new(),
                })
                .collect();
            ok_with(ResponsePayload::ProcessList(proto::ProcessList {
                processes: proto_processes,
            }))
        }
        Err(e) => {
            error!(error = ?e, "process enumeration failed");
            error_response(proto::ErrorCode::Internal, e.to_string())
        }
    }
}

fn data_flow_to_proto(flow: soundcore_audio::DataFlow) -> proto::DataFlow {
    match flow {
        soundcore_audio::DataFlow::Render => proto::DataFlow::Render,
        soundcore_audio::DataFlow::Capture => proto::DataFlow::Capture,
    }
}

fn error_response(code: proto::ErrorCode, message: String) -> proto::Response {
    proto::Response {
        payload: Some(ResponsePayload::Error(proto::ErrorReply {
            code: code.into(),
            message,
        })),
    }
}
