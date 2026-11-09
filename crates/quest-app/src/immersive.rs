//! Android immersive activity using DeskQuest's OpenXR context and surface swapchain.
use jni::{
    objects::{JByteArray, JObject, JString, JValue},
    sys::{jboolean, jfloat, jlong, jstring},
    JNIEnv,
};
use quest_jellyfin_player::{
    build_stereo_cylinder_layers_at_pose, build_stereo_quad_layers_at_pose,
    screen_controls::{
        artwork_height, panel_transform, subtitle_transform, Button, ScreenControls,
    },
    EyeOrder, Projection, StereoFormat, VirtualScreenConfig,
};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::Duration;
use xr_core::spatial::{
    HandGestureDetector, Handedness, InteractionInput, Quat, ScreenPose, ScreenTransform, Vec3,
};
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
static PLAYING: AtomicU8 = AtomicU8::new(0);
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
    PLAYING.store(0, Ordering::Release);
    EPOCH.fetch_add(1, Ordering::AcqRel).wrapping_add(1) as jlong
}

#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_VrActivity_nativePlaybackStatus(
    _: JNIEnv<'_>,
    _: JObject<'_>,
    epoch: jlong,
    playing: jboolean,
) {
    if EPOCH.load(Ordering::Acquire) == epoch as u64 {
        PLAYING.store(u8::from(playing != 0), Ordering::Release);
    }
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
    projection: JString<'_>,
    swap_eyes: jboolean,
    comfort_mono: jboolean,
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
            let (format, order) = crate::surface_size::parse_immersive_stereo(&format, &order)?;
            let projection: String = env
                .get_string(&projection)
                .map_err(|_| "Invalid projection")?
                .into();
            let projection = crate::surface_size::parse_projection(&projection)?;
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
                    projection,
                    swap_eyes: swap_eyes != 0,
                    comfort_mono: comfort_mono != 0,
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
    projection: Projection,
    swap_eyes: bool,
    comfort_mono: bool,
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
        mut order,
        mut projection,
        mut swap_eyes,
        mut comfort_mono,
        passthrough,
        screen_width,
        screen_distance,
        epoch,
    } = playback;
    if swap_eyes {
        order = crate::surface_size::swap_eye_order(order);
    }
    let spherical = projection != Projection::Flat;
    let passthrough = passthrough && !spherical;
    let mut view_tilt = 0.0_f32;
    let mut screen = VirtualScreenConfig {
        environment: quest_jellyfin_player::VirtualScreenEnvironment::from_passthrough(passthrough),
        screen_width_m: screen_width,
        distance_m: screen_distance,
        ..Default::default()
    };
    let config = XrConfig {
        enable_hud: true,
        ..screen.to_xr_config("Jellyfin 3D", width, height)
    };
    diagnostic("XR initializing");
    let mut vulkan = VulkanContext::new_simulated();
    let mut xr =
        unsafe { OpenXrContext::try_new_android(&config, &mut vulkan, activity.as_raw().cast()) }
            .map_err(|e| e.to_string())?;
    diagnostic("XR context created");
    if spherical && !xr.supports_equirect2() {
        return Err("This headset runtime does not support spherical video layers".into());
    }
    if passthrough && !xr.is_passthrough_enabled() {
        return Err("Passthrough is unavailable on this headset".into());
    }
    let mut size = crate::surface_size::SurfaceSize::new(width, height, format, order)?;
    let mut controls = ScreenControls::default();
    controls.stereo_alignment_supported =
        spherical && quest_jellyfin_player::is_packed_stereo(format);
    let mut alignment = quest_jellyfin_player::projection::StereoAlignment::from_degrees(
        env.call_method(activity, "initialAlignment", "()F", &[])
            .map_err(|_| "Cannot read stereo alignment")?
            .f()
            .map_err(|_| "Invalid stereo alignment")?,
    );
    let mut gestures = HandGestureDetector::default();
    let loft = !spherical
        && !passthrough
        && env
            .call_method(activity, "isLoft", "()Z", &[])
            .map_err(|_| "Cannot read cinema environment")?
            .z()
            .map_err(|_| "Invalid environment")?;
    let mut loft_anchor = None;
    let mut surface_started = false;
    let mut transform = ScreenTransform {
        position: Vec3::new(0.0, 0.0, -screen_distance),
        orientation: Quat::IDENTITY,
        quad_size: crate::surface_size::immersive_quad_size(screen_width, format, width, height),
        aspect_ratio: quest_jellyfin_player::calculate_display_aspect_ratio(format, width, height),
        distance: screen_distance,
        display_mode: xr_core::DisplayMode::FlatQuad,
        ..Default::default()
    };
    let curve = env
        .call_method(activity, "initialCurve", "()F", &[])
        .map_err(|_| "Cannot read screen curvature")?
        .f()
        .map_err(|_| "Invalid curvature")?;
    if !spherical && xr.supports_cylinder() && curve.is_finite() {
        quest_jellyfin_player::screen_controls::bend(&mut transform, curve.clamp(0.0, 2.4));
    }
    if spherical {
        transform.quad_size = (2.8, 1.6);
        transform.aspect_ratio = 2.8 / 1.6;
    }
    let mut anchored = false;
    let mut panorama_orientation = Quat::IDENTITY;
    let mut atlas_ready = false;
    let mut atlas_visual = (-1, -1, false, -1);
    let mut pressed_visual = (-1, std::time::Instant::now() - Duration::from_secs(1));
    let mut ui_swapchain = None;
    let mut subtitle_swapchain = None;
    let mut subtitle_cue = String::new();
    let mut subtitle_ready = false;
    let mut atlas_refresh = std::time::Instant::now() - Duration::from_secs(1);
    let mut frame_clock = std::time::Instant::now();
    let mut had_video_frame = false;
    let mut reported_controls = false;
    let mut last_diagnostic = std::time::Instant::now();
    let mut previous_session_state = None;
    let mut submitted_frames = 0_u64;
    let mut reported_video_layer = false;
    let mut reported_presentation = None;
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
                let focused = state == openxr::SessionState::FOCUSED;
                env.call_method(
                    activity,
                    "onXrFocusChanged",
                    "(JZ)V",
                    &[JValue::Long(epoch as jlong), JValue::Bool(focused as u8)],
                )
                .map_err(|_| "Cannot update headset presence")?;
                if !focused {
                    controls.suspend();
                }
            }
            if !xr.is_session_running() {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            if !xr.swapchains_created() {
                xr.create_swapchains(&config).map_err(|e| e.to_string())?;
                diagnostic("XR swapchains created");
                ui_swapchain = xr
                    .create_ui_swapchain(1536, 784)
                    .map_err(|e| e.to_string())?;
                subtitle_swapchain = xr
                    .create_ui_swapchain(1024, 256)
                    .map_err(|e| e.to_string())?;
                let pixels = env
                    .call_method(activity, "environmentPixels", "()[B", &[])
                    .map_err(|_| "Cannot draw room materials")?
                    .l()
                    .map_err(|_| "Invalid room materials")?;
                let bytes = JByteArray::from(pixels);
                let mut rgba = env
                    .convert_byte_array(&bytes)
                    .map_err(|_| "Invalid room pixels")?;
                env.delete_local_ref(bytes)
                    .map_err(|_| "Cannot release room pixels")?;
                if rgba.len() != 512 * 256 * 4 {
                    return Err("Invalid room atlas dimensions".into());
                }
                crate::cinema_atlas::add_handles(&mut rgba);
                crate::cinema_atlas::upload(
                    xr.hud_swapchain().ok_or("No room swapchain")?,
                    &vulkan,
                    &rgba,
                )?;
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
                        let w = transform.size_m().0;
                        let aspect = quest_jellyfin_player::calculate_display_aspect_ratio(
                            format, width, height,
                        );
                        if !spherical {
                            transform.quad_size =
                                crate::surface_size::immersive_quad_size(w, format, width, height);
                            transform.aspect_ratio = aspect;
                        }
                        had_video_frame = false;
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
            let head = xr.head_pose_at(frame.predicted_display_time);
            if loft && loft_anchor.is_none() {
                if let Some(head) = head {
                    let forward = head.orientation.rotate_vec(-Vec3::UNIT_Z);
                    let yaw = (-forward.x).atan2(-forward.z);
                    loft_anchor = Some(ScreenPose::new(
                        Vec3::new(head.position.x, 0.0, head.position.z),
                        Quat::from_axis_angle(Vec3::UNIT_Y, yaw),
                    ));
                }
            }
            if !anchored {
                if let Some(head) = head {
                    let pose =
                        quest_jellyfin_player::cinema::pose_in_front(head, screen.distance_m);
                    transform.position = pose.position;
                    transform.orientation = pose.orientation;
                    panorama_orientation = pose.orientation;
                    anchored = true;
                }
            }
            let focused = state == openxr::SessionState::FOCUSED;
            let mut inputs = Vec::new();
            if focused && head.is_some() {
                for hand in [Handedness::Left, Handedness::Right] {
                    let controller = xr.interaction().controller_state(hand);
                    let joints = xr
                        .locate_hand_joints(hand, frame.predicted_display_time)
                        .map_err(|e| e.to_string())?;
                    if let Some(joints) = joints.filter(|j| j.is_active) {
                        let gesture = gestures.update(hand, &joints);
                        inputs.push(InteractionInput::from_tracked_hand(
                            hand,
                            &gesture,
                            controller.as_ref(),
                        ));
                    } else if let Some(controller) = controller {
                        inputs.push(InteractionInput::from_controller(hand, &controller));
                    }
                }
            }
            let dt = frame_clock.elapsed().as_secs_f32().min(0.05);
            frame_clock = std::time::Instant::now();
            if spherical && !controls.visible && inputs.iter().any(|i| i.is_selecting) {
                if let Some(head) = head {
                    let pose =
                        quest_jellyfin_player::cinema::pose_in_front(head, screen.distance_m);
                    transform.position = pose.position;
                    transform.orientation = pose.orientation;
                }
                controls.suspend();
            }
            let previous_projection = projection;
            let previous_tilt = view_tilt;
            let before = transform.clone();
            let was_grabbing = controls.grab.is_grabbing();
            let command = head
                .filter(|_| focused)
                .and_then(|head| controls.update(&inputs, head, &mut transform, dt));
            if !focused || head.is_none() {
                controls.suspend();
            }
            if let Some(command) = command {
                pressed_visual = (command.artwork_id(), std::time::Instant::now());
                let code = match command {
                    Button::Seek(fraction) => {
                        env.call_method(
                            activity,
                            "onXrSeek",
                            "(I)V",
                            &[JValue::Int(i32::from(fraction))],
                        )
                        .map_err(|_| "Cannot seek video")?;
                        None
                    }
                    Button::PlayPause => Some(0),
                    Button::Back => Some(1),
                    Button::Forward => Some(2),
                    Button::Audio => Some(3),
                    Button::Subtitles => Some(5),
                    Button::SwapEyes | Button::ComfortMono => {
                        if command == Button::SwapEyes {
                            swap_eyes = !swap_eyes;
                            order = crate::surface_size::swap_eye_order(order);
                        } else {
                            comfort_mono = !comfort_mono;
                        }
                        env.call_method(
                            activity,
                            "onXrStereoChanged",
                            "(JZZ)V",
                            &[
                                JValue::Long(epoch as jlong),
                                JValue::Bool(u8::from(swap_eyes)),
                                JValue::Bool(u8::from(comfort_mono)),
                            ],
                        )
                        .map_err(|_| "Cannot update stereo view")?;
                        None
                    }
                    Button::AlignMinus | Button::AlignPlus | Button::ResetAlignment => {
                        match command {
                            Button::AlignMinus => alignment.adjust_steps(-1),
                            Button::AlignPlus => alignment.adjust_steps(1),
                            _ => alignment = Default::default(),
                        }
                        env.call_method(
                            activity,
                            "onXrAlignmentChanged",
                            "(JF)V",
                            &[
                                JValue::Long(epoch as jlong),
                                JValue::Float(alignment.degrees()),
                            ],
                        )
                        .map_err(|_| "Cannot save stereo alignment")?;
                        None
                    }
                    Button::Exit => Some(4),
                    Button::Smaller => {
                        if spherical {
                            view_tilt =
                                (view_tilt - 5_f32.to_radians()).max(-std::f32::consts::FRAC_PI_2);
                        } else {
                            quest_jellyfin_player::screen_controls::resize(&mut transform, 0.9);
                        }
                        None
                    }
                    Button::Larger => {
                        if spherical {
                            view_tilt =
                                (view_tilt + 5_f32.to_radians()).min(std::f32::consts::FRAC_PI_2);
                        } else {
                            quest_jellyfin_player::screen_controls::resize(&mut transform, 1.1);
                        }
                        None
                    }
                    Button::Flatter => {
                        if spherical {
                            projection = Projection::Equirectangular180;
                        } else {
                            quest_jellyfin_player::screen_controls::bend(&mut transform, -0.3);
                        }
                        None
                    }
                    Button::Curvier => {
                        if spherical {
                            projection = Projection::Equirectangular360;
                        } else if xr.supports_cylinder() {
                            quest_jellyfin_player::screen_controls::bend(&mut transform, 0.3);
                        }
                        None
                    }
                    Button::Recenter => {
                        view_tilt = 0.0;
                        controls.grab.release();
                        if let Some(head) = head {
                            let pose = quest_jellyfin_player::cinema::pose_in_front(
                                head,
                                screen.distance_m,
                            );
                            transform.position = pose.position;
                            transform.orientation = pose.orientation;
                            panorama_orientation = pose.orientation;
                            anchored = true;
                        } else {
                            anchored = false;
                        }
                        None
                    }
                    Button::Hide | Button::Screen => None,
                };
                if let Some(code) = code {
                    env.call_method(activity, "onXrCommand", "(I)V", &[JValue::Int(code)])
                        .map_err(|_| "Cannot send playback control")?;
                }
            }
            if spherical && (projection != previous_projection || view_tilt != previous_tilt) {
                let coverage = match projection {
                    Projection::Equirectangular180 => 180,
                    Projection::Equirectangular360 => 360,
                    Projection::Fisheye => 0,
                    Projection::Flat => 0,
                };
                env.call_method(
                    activity,
                    "onXrProjectionChanged",
                    "(JIF)V",
                    &[
                        JValue::Long(epoch as jlong),
                        JValue::Int(coverage),
                        JValue::Float(view_tilt.to_degrees()),
                    ],
                )
                .map_err(|_| "Cannot update VR view settings")?;
            }
            if !spherical && (transform != before || was_grabbing) && !controls.grab.is_grabbing() {
                let distance = head
                    .map(|h| (transform.position - h.position).length())
                    .unwrap_or(screen.distance_m);
                env.call_method(
                    activity,
                    "onXrScreenChanged",
                    "(FFF)V",
                    &[
                        JValue::Float(transform.size_m().0),
                        JValue::Float(distance),
                        JValue::Float(
                            if transform.display_mode == xr_core::DisplayMode::CurvedCylinder {
                                transform.curve_angle
                            } else {
                                0.0
                            },
                        ),
                    ],
                )
                .map_err(|_| "Cannot save screen preferences")?;
            }
            controls.advance(PLAYING.load(Ordering::Acquire) != 0 && had_video_frame, dt);
            if !had_video_frame || VIDEO_STATE.load(Ordering::Acquire) != 1 {
                controls.visible = true;
                controls.opacity = 1.0;
            }
            if last_diagnostic.elapsed() >= Duration::from_secs(2) {
                diagnostic(&format!(
                    "XR frame render={} tracked={} video_state={} submitted={submitted_frames}",
                    frame.should_render,
                    anchored,
                    VIDEO_STATE.load(Ordering::Acquire)
                ));
                last_diagnostic = std::time::Instant::now();
            }
            if !frame.should_render {
                xr.end_frame_empty(frame.predicted_display_time)
                    .map_err(|e| e.to_string())?;
                continue;
            }
            // A regular HUD swapchain remains available while the surface decoder is loading,
            // paused, or buffering. Transport never depends on the decoder's first frame.
            let visual = (
                controls.hovered.map(Button::artwork_id).unwrap_or(-1),
                if pressed_visual.1.elapsed() < Duration::from_millis(140) {
                    pressed_visual.0
                } else {
                    -1
                },
                controls.tools_open,
                (controls.opacity * 20.0).round() as i32,
            );
            if (controls.opacity > 0.001 && atlas_refresh.elapsed() >= Duration::from_millis(250))
                || !atlas_ready
                || visual != atlas_visual
            {
                let pixels = env
                    .call_method(
                        activity,
                        "controlsPixelsWithState",
                        "(IIZF)[B",
                        &[
                            JValue::Int(visual.0),
                            JValue::Int(visual.1),
                            JValue::Bool(u8::from(visual.2)),
                            JValue::Float(controls.opacity),
                        ],
                    )
                    .map_err(|_| "Cannot draw cinema controls")?
                    .l()
                    .map_err(|_| "Invalid cinema controls")?;
                let bytes = JByteArray::from(pixels);
                let rgba = env
                    .convert_byte_array(&bytes)
                    .map_err(|_| "Invalid control pixels")?;
                env.delete_local_ref(bytes)
                    .map_err(|_| "Cannot release control pixels")?;
                if rgba.len() != 1536 * 784 * 4 {
                    return Err("Invalid cinema controls dimensions".into());
                }
                crate::cinema_atlas::upload_sized(
                    ui_swapchain.as_ref().ok_or("No control swapchain")?,
                    &vulkan,
                    &rgba,
                    1536,
                    784,
                )?;
                atlas_ready = true;
                atlas_visual = visual;
                atlas_refresh = std::time::Instant::now();
            }
            let cue = {
                let value = JString::from(
                    env.call_method(activity, "currentSubtitleCue", "()Ljava/lang/String;", &[])
                        .map_err(|_| "Cannot read subtitle cue")?
                        .l()
                        .map_err(|_| "Invalid subtitle cue")?,
                );
                let text: String = env
                    .get_string(&value)
                    .map_err(|_| "Invalid subtitle cue text")?
                    .into();
                env.delete_local_ref(value)
                    .map_err(|_| "Cannot release subtitle cue")?;
                text
            };
            if cue != subtitle_cue {
                subtitle_cue = cue;
                if subtitle_cue.is_empty() {
                    subtitle_ready = false;
                } else {
                    let pixels = env
                        .call_method(activity, "subtitlePixels", "()[B", &[])
                        .map_err(|_| "Cannot draw subtitle cue")?
                        .l()
                        .map_err(|_| "Invalid subtitle pixels")?;
                    let bytes = JByteArray::from(pixels);
                    let rgba = env
                        .convert_byte_array(&bytes)
                        .map_err(|_| "Invalid subtitle pixel bytes")?;
                    env.delete_local_ref(bytes)
                        .map_err(|_| "Cannot release subtitle pixels")?;
                    if rgba.len() != 1024 * 256 * 4 {
                        return Err("Invalid subtitle atlas dimensions".into());
                    }
                    crate::cinema_atlas::upload_sized(
                        subtitle_swapchain.as_ref().ok_or("No subtitle swapchain")?,
                        &vulkan,
                        &rgba,
                        1024,
                        256,
                    )?;
                    subtitle_ready = true;
                }
            }
            if VIDEO_STATE.load(Ordering::Acquire) == 1 {
                had_video_frame = true;
            }
            let present_video = had_video_frame && VIDEO_STATE.load(Ordering::Acquire) == 1;
            // Keep consuming the Android Surface while waiting. Omitting its layers
            // fills the producer queue and can prevent the first frame / seek from
            // completing. A later opaque layer hides every eye until ready.
            let space = if anchored {
                xr.stage_space()
            } else {
                xr.view_space()
            }
            .ok_or("No OpenXR cinema reference space")?;
            let hud = xr.hud_swapchain().ok_or("No cinema control swapchain")?;
            let swapchain = xr.desktop_swapchain().ok_or("No OpenXR video swapchain")?;
            screen.screen_width_m = transform.size_m().0;
            let quad_layers = if surface_started
                && !spherical
                && transform.display_mode == xr_core::DisplayMode::FlatQuad
            {
                build_stereo_quad_layers_at_pose(
                    space,
                    swapchain,
                    format,
                    order,
                    size.width,
                    size.height,
                    &screen,
                    transform.pose().into(),
                )
                .map_err(|e| e.to_string())?
            } else {
                Vec::new()
            };
            let quad_layers = if comfort_mono {
                quad_layers
                    .into_iter()
                    .take(1)
                    .map(|layer| layer.eye_visibility(openxr::EyeVisibility::BOTH))
                    .collect::<Vec<_>>()
            } else {
                quad_layers
            };
            let cylinder_layers = if surface_started
                && !spherical
                && transform.display_mode == xr_core::DisplayMode::CurvedCylinder
            {
                build_stereo_cylinder_layers_at_pose(
                    space,
                    swapchain,
                    format,
                    order,
                    size.width,
                    size.height,
                    transform.cylinder_axis_pose().into(),
                    transform.curve_radius,
                    transform.curve_angle,
                    transform.aspect_ratio,
                )
                .map_err(|e| e.to_string())?
            } else {
                Vec::new()
            };
            let cylinder_layers = if comfort_mono {
                cylinder_layers
                    .into_iter()
                    .take(1)
                    .map(|layer| layer.eye_visibility(openxr::EyeVisibility::BOTH))
                    .collect::<Vec<_>>()
            } else {
                cylinder_layers
            };
            let equirect_layers = if surface_started && spherical {
                let orientation =
                    panorama_orientation * Quat::from_axis_angle(Vec3::UNIT_X, view_tilt);
                let pose: openxr::Posef = ScreenPose::new(Vec3::ZERO, orientation).into();
                quest_jellyfin_player::projection::build_equirect_layers_aligned(
                    space,
                    swapchain,
                    format,
                    order,
                    size.width,
                    size.height,
                    projection,
                    pose,
                    comfort_mono,
                    alignment,
                )
                .map_err(|e| e.to_string())?
            } else {
                Vec::new()
            };
            let view_space = xr.view_space().ok_or("No cinema view reference space")?;
            let waiting_cover = (!present_video).then(|| {
                crate::cinema_atlas::quad(
                    view_space,
                    hud,
                    (496, 0, 16, 16),
                    ScreenPose::new(Vec3::new(0.0, 0.0, -0.5), Quat::IDENTITY),
                    (16.0, 16.0),
                )
            });
            let mut overlays = Vec::new();
            let room = loft_anchor
                .map(quest_jellyfin_player::environment::loft_surfaces)
                .unwrap_or_default();
            let room_layers: Vec<_> = room
                .iter()
                .map(|surface| {
                    crate::cinema_atlas::quad(space, hud, surface.rect, surface.pose, surface.size)
                })
                .collect();
            let panel = panel_transform(&transform, controls.tools_open);
            let mut control_cylinders = Vec::new();
            let mut subtitle_cylinders = Vec::new();
            // Cues stay attached to the cinema/workspace even while transport chrome fades.
            if subtitle_ready && !subtitle_cue.is_empty() && present_video {
                let cue_panel = subtitle_transform(&transform);
                let rect = (0, 0, 1024, 256);
                let texture = subtitle_swapchain.as_ref().ok_or("No subtitle texture")?;
                if cue_panel.display_mode == xr_core::DisplayMode::FlatQuad {
                    overlays.push(crate::cinema_atlas::quad(
                        space,
                        texture,
                        rect,
                        cue_panel.pose(),
                        cue_panel.size_m(),
                    ));
                } else {
                    subtitle_cylinders.push(crate::cinema_atlas::cylinder(
                        space, texture, rect, &cue_panel,
                    ));
                }
            }
            if controls.opacity > 0.001 || !had_video_frame {
                let rect = (0, 0, 1536, artwork_height(controls.tools_open) * 2);
                let control_texture = ui_swapchain.as_ref().ok_or("No control texture")?;
                if panel.display_mode == xr_core::DisplayMode::FlatQuad {
                    overlays.push(crate::cinema_atlas::quad(
                        space,
                        control_texture,
                        rect,
                        panel.pose(),
                        panel.size_m(),
                    ));
                } else {
                    control_cylinders.push(crate::cinema_atlas::cylinder(
                        space,
                        control_texture,
                        rect,
                        &panel,
                    ));
                }
            }
            if had_video_frame
                && (controls.tools_open
                    || controls.grab.is_grabbing()
                    || controls.grab.hovered_corner().is_some()
                    || controls.grab.hovering_handle())
            {
                // Screen tools reveal handles; otherwise show only the handle under the pointer.
                for corner in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                    let hovered = controls.grab.hovered_corner().map(|c| match c {
                        xr_core::spatial::Corner::TopLeft => (0.0, 0.0),
                        xr_core::spatial::Corner::TopRight => (1.0, 0.0),
                        xr_core::spatial::Corner::BottomLeft => (0.0, 1.0),
                        xr_core::spatial::Corner::BottomRight => (1.0, 1.0),
                    });
                    if !controls.tools_open
                        && !controls.grab.is_grabbing()
                        && hovered != Some(corner)
                    {
                        continue;
                    }
                    let chrome = xr_core::openxr::overlay::window_chrome(
                        &transform,
                        Some(corner),
                        false,
                        Some(corner),
                    );
                    if let Some(handle) = chrome.corner {
                        overlays.push(crate::cinema_atlas::quad(
                            space,
                            hud,
                            (64, 192, 64, 64),
                            handle.pose,
                            handle.size,
                        ));
                    }
                }
                let chrome = xr_core::openxr::overlay::window_chrome(
                    &transform,
                    Some((0.5, 1.05)),
                    true,
                    None,
                );
                if let Some(handle) = chrome.bar {
                    overlays.push(crate::cinema_atlas::quad(
                        space,
                        hud,
                        (128, 192, 128, 12),
                        handle.pose,
                        handle.size,
                    ));
                }
            }
            if head.is_some() && focused {
                for input in &inputs {
                    if let Some(ray) = input.aim_ray {
                        let panel_hit = controls
                            .visible
                            .then(|| panel.intersect_ray(&ray))
                            .flatten();
                        let screen_hit =
                            ScreenTransform::find_closest_window_hit(&ray, [(0, &transform)])
                                .map(|(_, hit)| hit);
                        if let Some(hit) = panel_hit.or(screen_hit) {
                            let pointer = xr_core::openxr::overlay::PointerVisual {
                                origin: ray.origin,
                                direction: ray.direction,
                                length: hit.distance,
                                hit: true,
                            };
                            overlays.push(crate::cinema_atlas::quad(
                                xr.stage_space().ok_or("No pointer reference space")?,
                                hud,
                                (0, 192, 48, 48),
                                xr_core::openxr::overlay::cursor_pose(&pointer),
                                (0.024, 0.024),
                            ));
                        }
                    }
                }
            }
            let pt_layer = screen
                .environment
                .uses_passthrough()
                .then(|| xr.passthrough().passthrough_layer())
                .flatten()
                .filter(|_| xr.is_passthrough_enabled())
                .map(xr_core::build_passthrough_layer::<openxr::Vulkan>);
            let mut refs: Vec<&openxr::CompositionLayerBase<'_, openxr::Vulkan>> = Vec::new();
            if let Some(ref layer) = pt_layer {
                refs.push(&**layer);
            }
            refs.extend(room_layers.iter().map(|layer| &**layer));
            refs.extend(quad_layers.iter().map(|layer| &**layer));
            refs.extend(cylinder_layers.iter().map(|layer| &**layer));
            refs.extend(equirect_layers.iter().map(|layer| &**layer));
            if let Some(ref cover) = waiting_cover {
                refs.push(&**cover);
            }
            refs.extend(subtitle_cylinders.iter().map(|layer| &**layer));
            refs.extend(control_cylinders.iter().map(|layer| &**layer));
            refs.extend(overlays.iter().map(|layer| &**layer));
            xr.end_frame(frame.predicted_display_time, &refs)
                .map_err(|e| e.to_string())?;
            let presentation = (
                if present_video {
                    quad_layers.len() + cylinder_layers.len() + equirect_layers.len()
                } else {
                    0
                },
                comfort_mono,
            );
            if reported_presentation != Some(presentation) {
                env.call_method(
                    activity,
                    "onXrPresentationChanged",
                    "(JIZ)V",
                    &[
                        JValue::Long(epoch as jlong),
                        JValue::Int(presentation.0 as i32),
                        JValue::Bool(u8::from(comfort_mono)),
                    ],
                )
                .map_err(|_| "Cannot confirm current stereo presentation")?;
                reported_presentation = Some(presentation);
            }
            submitted_frames += 1;
            if !reported_controls {
                env.call_method(
                    activity,
                    "onXrControlsSubmitted",
                    "(J)V",
                    &[JValue::Long(epoch as jlong)],
                )
                .map_err(|_| "Cannot confirm controls presentation")?;
                diagnostic("XR controls layer submitted");
                reported_controls = true;
            }
            if presentation.0 > 0 && !reported_video_layer {
                if !equirect_layers.is_empty() {
                    env.call_method(
                        activity,
                        "onXrSphericalLayerSubmitted",
                        "(J)V",
                        &[JValue::Long(epoch as jlong)],
                    )
                    .map_err(|_| "Cannot confirm spherical presentation")?;
                }
                env.call_method(
                    activity,
                    "onXrVideoLayerSubmitted",
                    "(J)V",
                    &[JValue::Long(epoch as jlong)],
                )
                .map_err(|_| "Cannot confirm cinema presentation")?;
                diagnostic(if spherical {
                    "XR first spherical video layers submitted"
                } else {
                    "XR first video layer submitted"
                });
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
