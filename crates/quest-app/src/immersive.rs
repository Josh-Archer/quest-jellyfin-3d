//! Android immersive activity using DeskQuest's OpenXR context and surface swapchain.
use jni::{
    objects::{JByteArray, JObject, JString, JValue},
    sys::{jboolean, jfloat, jlong, jstring},
    JNIEnv,
};
use quest_jellyfin_player::{
    build_stereo_quad_layers_at_pose, EyeOrder, StereoFormat, VirtualScreenConfig,
};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::Duration;
use xr_core::{OpenXrContext, XrConfig};
use xr_video::graphics::vulkan::VulkanContext;

#[link(name = "log")]
extern "C" {
    fn __android_log_write(
        priority: i32,
        tag: *const std::ffi::c_char,
        text: *const std::ffi::c_char,
    ) -> i32;
}
fn diagnostic(message: &str) {
    if let Ok(text) = std::ffi::CString::new(message) {
        unsafe {
            __android_log_write(4, c"Jellyfin3D".as_ptr(), text.as_ptr());
        }
    }
}

static EPOCH: AtomicU64 = AtomicU64::new(0);
static VIDEO_STATE: AtomicU8 = AtomicU8::new(0);
static XR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static VIDEO_SIZE: std::sync::Mutex<Option<(u64, u32, u32)>> = std::sync::Mutex::new(None);

struct AndroidContextGuard;
impl Drop for AndroidContextGuard {
    fn drop(&mut self) {
        unsafe {
            ndk_context::release_android_context();
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativeStop(
    _: JNIEnv<'_>,
    _: JObject<'_>,
) {
    EPOCH.fetch_add(1, Ordering::AcqRel);
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativeArm(
    _: JNIEnv<'_>,
    _: JObject<'_>,
) -> jlong {
    VIDEO_STATE.store(0, Ordering::Release);
    EPOCH.fetch_add(1, Ordering::AcqRel).wrapping_add(1) as jlong
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativeVideoState(
    _: JNIEnv<'_>,
    _: JObject<'_>,
    epoch: jlong,
    state: i32,
) {
    if EPOCH.load(Ordering::Acquire) == epoch as u64 {
        VIDEO_STATE.store(state.clamp(0, 2) as u8, Ordering::Release);
    }
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativeVideoSize(
    _: JNIEnv<'_>,
    _: JObject<'_>,
    epoch: jlong,
    width: i32,
    height: i32,
) {
    if EPOCH.load(Ordering::Acquire) != epoch as u64 {
        return;
    }
    if let (Ok(width), Ok(height), Ok(mut pending)) = (
        u32::try_from(width),
        u32::try_from(height),
        VIDEO_SIZE.lock(),
    ) {
        if width > 0 && height > 0 {
            *pending = Some((epoch as u64, width, height));
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativeRun(
    mut env: JNIEnv<'_>,
    activity: JObject<'_>,
    width: i32,
    height: i32,
    format: JString<'_>,
    order: JString<'_>,
    passthrough: jboolean,
    screen_width: jfloat,
    screen_distance: jfloat,
    epoch: jlong,
) -> jstring {
    let _guard = match XR_LOCK.lock() {
        Ok(guard) => guard,
        Err(_) => return std::ptr::null_mut(),
    };
    if EPOCH.load(Ordering::Acquire) != epoch as u64 {
        return env
            .new_string("")
            .map(|v| v.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let format: String = env
                .get_string(&format)
                .map_err(|_| "Invalid format")?
                .into();
            let order: String = env
                .get_string(&order)
                .map_err(|_| "Invalid eye order")?
                .into();
            let format = match format.as_str() {
                "Mono" => StereoFormat::Mono,
                "HalfSideBySide" => StereoFormat::HalfSideBySide,
                "FullSideBySide" => StereoFormat::FullSideBySide,
                "HalfOverUnder" => StereoFormat::HalfOverUnder,
                "FullOverUnder" => StereoFormat::FullOverUnder,
                _ => return Err("This stereo format is not supported yet".into()),
            };
            let order = match order.as_str() {
                "RightLeft" => EyeOrder::RightLeft,
                "TopBottom" => EyeOrder::TopBottom,
                "BottomTop" => EyeOrder::BottomTop,
                _ => EyeOrder::LeftRight,
            };
            let (width, height) = (
                u32::try_from(width).map_err(|_| "Invalid video width")?,
                u32::try_from(height).map_err(|_| "Invalid video height")?,
            );
            // Validate packing before creating a compositor surface.
            crate::surface_size::SurfaceSize::new(width, height, format, order)?;
            let vm = env.get_java_vm().map_err(|_| "No Android VM")?;
            let global = env
                .new_global_ref(&activity)
                .map_err(|_| "Cannot retain activity")?;
            unsafe {
                ndk_context::initialize_android_context(
                    vm.get_java_vm_pointer().cast(),
                    global.as_obj().as_raw().cast(),
                );
            }
            let _context_guard = AndroidContextGuard;
            run_xr(
                &mut env,
                global.as_obj(),
                XrPlayback {
                    width,
                    height,
                    format,
                    order,
                    passthrough: passthrough != 0,
                    screen_width: if screen_width.is_finite() {
                        screen_width.clamp(1.5, 6.0)
                    } else {
                        4.0
                    },
                    screen_distance: if screen_distance.is_finite() {
                        screen_distance.clamp(1.0, 5.0)
                    } else {
                        2.2
                    },
                    epoch: epoch as u64,
                },
            )
        }));
    let message = match result {
        Ok(Ok(())) => String::new(),
        Ok(Err(e)) => e,
        Err(_) => "OpenXR initialization failed".into(),
    };
    if !message.is_empty() {
        diagnostic(&format!("XR failure: {message}"));
    }
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    env.new_string(message)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

struct XrPlayback {
    width: u32,
    height: u32,
    format: StereoFormat,
    order: EyeOrder,
    passthrough: bool,
    screen_width: f32,
    screen_distance: f32,
    epoch: u64,
}

fn run_xr(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    playback: XrPlayback,
) -> Result<(), String> {
    let XrPlayback {
        width,
        height,
        format,
        order,
        passthrough,
        screen_width,
        screen_distance,
        epoch,
    } = playback;
    let config = XrConfig {
        application_name: "Jellyfin 3D".into(),
        stream_width: width,
        stream_height: height,
        enable_passthrough: passthrough,
        enable_hud: true,
        ..Default::default()
    };
    diagnostic("XR initializing");
    let mut vulkan = VulkanContext::new_simulated();
    let mut xr =
        unsafe { OpenXrContext::try_new_android(&config, &mut vulkan, activity.as_raw().cast()) }
            .map_err(|e| e.to_string())?;
    diagnostic("XR context created");
    if passthrough && !xr.is_passthrough_enabled() {
        return Err("Passthrough is unavailable on this headset".into());
    }
    let mut images = Vec::new();
    for index in 0..8 {
        let bytes = env
            .call_method(activity, "loadingPixels", "(I)[B", &[JValue::Int(index)])
            .map_err(|_| "Cannot create loading display")?
            .l()
            .map_err(|_| "Invalid loading display")?;
        let rgba = env
            .convert_byte_array(JByteArray::from(bytes))
            .map_err(|_| "Invalid loading pixels")?;
        if rgba.len() != 512 * 256 * 4 {
            return Err("Invalid loading display dimensions".into());
        }
        images.push(rgba);
    }
    let mut size = crate::surface_size::SurfaceSize::new(width, height, format, order)?;
    let mut controls = crate::controls::PlaybackControls::default();
    let mut surface_started = false;
    let screen = VirtualScreenConfig {
        screen_width_m: screen_width,
        distance_m: screen_distance,
        ..Default::default()
    };
    let mut cinema_pose = None;
    let mut recenter_down = false;
    let animation_start = std::time::Instant::now();
    let mut last_diagnostic = std::time::Instant::now();
    let mut previous_session_state = None;
    let mut submitted_frames = 0_u64;
    let mut reported_video_layer = false;
    let render_result = (|| -> Result<(), String> {
        while EPOCH.load(Ordering::Acquire) == epoch && !xr.should_exit() {
            xr.poll_events().map_err(|e| e.to_string())?;
            let state = xr.session_state();
            if previous_session_state != Some(state) {
                diagnostic(&format!(
                    "XR session {state:?}, running={}",
                    xr.is_session_running()
                ));
                previous_session_state = Some(state);
            }
            if !xr.is_session_running() {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            if !xr.swapchains_created() {
                xr.create_swapchains(&config).map_err(|e| e.to_string())?;
                diagnostic("XR swapchains created");
            }
            if surface_started {
                let pending = VIDEO_SIZE.lock().ok().and_then(|mut value| value.take());
                if let Some((generation, width, height)) = pending {
                    if generation == epoch && size.requires_resize(width, height) {
                        let next =
                            crate::surface_size::SurfaceSize::new(width, height, format, order)?;
                        let detached = env
                            .call_method(activity, "onXrDetachSurface", "()Z", &[])
                            .map_err(|_| "Cannot detach video surface")?
                            .z()
                            .map_err(|_| "Invalid detach result")?;
                        if !detached {
                            return Err("Cannot detach video surface".into());
                        }
                        xr.resize_desktop_surface((width, height))
                            .map_err(|e| e.to_string())?;
                        size = next;
                        surface_started = false;
                    }
                }
            }
            if !surface_started {
                let window = xr
                    .surface_window()
                    .ok_or("OpenXR did not provide a video surface")?;
                let surface = unsafe {
                    ndk_sys::ANativeWindow_toSurface(env.get_raw().cast(), window.ptr().as_ptr())
                };
                if surface.is_null() {
                    return Err("Cannot obtain Android video surface".into());
                }
                let surface = unsafe { JObject::from_raw(surface.cast()) };
                env.call_method(
                    activity,
                    "onXrSurface",
                    "(Landroid/view/Surface;)V",
                    &[JValue::Object(&surface)],
                )
                .map_err(|_| "Cannot start video")?;
                surface_started = true;
                diagnostic("XR video surface delivered");
            }
            let frame = xr.wait_frame().map_err(|e| e.to_string())?;
            xr.begin_frame().map_err(|e| e.to_string())?;
            xr.poll_interaction(frame.predicted_display_time)
                .map_err(|e| e.to_string())?;
            use xr_core::spatial::interaction::Handedness;
            let left = xr.interaction().controller_state(Handedness::Left);
            let right = xr.interaction().controller_state(Handedness::Right);
            let trigger = [left, right]
                .into_iter()
                .flatten()
                .any(|s| s.is_selecting());
            let stick = [left, right]
                .into_iter()
                .flatten()
                .map(|s| s.thumbstick.0)
                .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                .unwrap_or(0.0);
            let grip = [left, right].into_iter().flatten().any(|s| s.is_grabbing());
            let recenter = [left, right]
                .into_iter()
                .flatten()
                .any(|s| s.thumbstick.1 > 0.75);
            if recenter && !recenter_down {
                cinema_pose = None;
            }
            recenter_down = recenter;
            if cinema_pose.is_none() {
                cinema_pose = xr.head_pose_at(frame.predicted_display_time).map(|head| {
                    quest_jellyfin_player::cinema::pose_in_front(head, screen.distance_m)
                });
            }
            if let Some(command) = controls.update(trigger, stick, grip) {
                let code = match command {
                    crate::controls::Command::TogglePlayback => 0,
                    crate::controls::Command::SeekBack => 1,
                    crate::controls::Command::SeekForward => 2,
                    crate::controls::Command::NextAudioTrack => 3,
                };
                env.call_method(activity, "onXrCommand", "(I)V", &[JValue::Int(code)])
                    .map_err(|_| "Cannot send playback control")?;
            }
            if last_diagnostic.elapsed() >= Duration::from_secs(2) {
                diagnostic(&format!(
                    "XR frame render={} tracked={} video={} submitted={submitted_frames}",
                    frame.should_render,
                    cinema_pose.is_some(),
                    VIDEO_STATE.load(Ordering::Acquire)
                ));
                last_diagnostic = std::time::Instant::now();
            }
            if !frame.should_render {
                xr.end_frame_empty(frame.predicted_display_time)
                    .map_err(|e| e.to_string())?;
                continue;
            }
            // Loading must remain visible even before positional tracking is available.
            if VIDEO_STATE.load(Ordering::Acquire) != 1 {
                let phase = (animation_start.elapsed().as_millis() / 100 % 8) as usize;
                xr.render_and_present_layers(
                    frame.predicted_display_time,
                    &config,
                    &vulkan,
                    Some(&images[phase]),
                    &xr_core::openxr::overlay::OverlayScene::default(),
                )
                .map_err(|e| e.to_string())?;
                submitted_frames += 1;
                continue;
            }
            let (reference, pose) =
                quest_jellyfin_player::cinema::select_pose(cinema_pose, screen.distance_m);
            let space = match reference {
                quest_jellyfin_player::cinema::CinemaReference::Stage => xr.stage_space(),
                quest_jellyfin_player::cinema::CinemaReference::View => xr.view_space(),
            }
            .ok_or("No OpenXR cinema reference space")?;
            let swapchain = xr.desktop_swapchain().ok_or("No OpenXR video swapchain")?;
            let layers = build_stereo_quad_layers_at_pose(
                space,
                swapchain,
                format,
                order,
                size.width,
                size.height,
                &screen,
                pose.into(),
            )
            .map_err(|e| e.to_string())?;
            let pt_layer = xr
                .passthrough()
                .passthrough_layer()
                .filter(|_| xr.is_passthrough_enabled())
                .map(xr_core::build_passthrough_layer::<openxr::Vulkan>);
            let mut refs: Vec<&openxr::CompositionLayerBase<'_, openxr::Vulkan>> = Vec::new();
            if let Some(ref layer) = pt_layer {
                refs.push(&**layer);
            }
            refs.extend(layers.iter().map(|layer| &**layer));
            xr.end_frame(frame.predicted_display_time, &refs)
                .map_err(|e| e.to_string())?;
            submitted_frames += 1;
            if !reported_video_layer {
                env.call_method(
                    activity,
                    "onXrVideoLayerSubmitted",
                    "(J)V",
                    &[JValue::Long(epoch as jlong)],
                )
                .map_err(|_| "Cannot confirm cinema presentation")?;
                diagnostic("XR first video layer submitted");
                reported_video_layer = true;
            }
        }
        Ok(())
    })();
    // Stop the media producer before destroying its OpenXR surface, including on errors.
    if surface_started {
        env.call_method(activity, "onXrClosing", "()V", &[])
            .map_err(|_| "Cannot stop video")?;
    }
    render_result
}
