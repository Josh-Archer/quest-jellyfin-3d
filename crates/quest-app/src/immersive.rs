//! Android immersive activity using DeskQuest's OpenXR context and surface swapchain.
use jni::{
    objects::{JObject, JString, JValue},
    sys::{jboolean, jlong, jstring},
    JNIEnv,
};
use quest_jellyfin_player::{
    build_stereo_quad_layers, EyeOrder, StereoFormat, VirtualScreenConfig,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use xr_core::{OpenXrContext, XrConfig};
use xr_video::graphics::vulkan::VulkanContext;

static EPOCH: AtomicU64 = AtomicU64::new(0);
static XR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    EPOCH.fetch_add(1, Ordering::AcqRel).wrapping_add(1) as jlong
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
            quest_jellyfin_player::build_stereo_eye_layer_specs(format, order, width, height)
                .map_err(|e| e.to_string())?;
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
                    epoch: epoch as u64,
                },
            )
        }));
    let message = match result {
        Ok(Ok(())) => String::new(),
        Ok(Err(e)) => e,
        Err(_) => "OpenXR initialization failed".into(),
    };
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
        epoch,
    } = playback;
    let config = XrConfig {
        application_name: "Jellyfin 3D".into(),
        stream_width: width,
        stream_height: height,
        enable_passthrough: passthrough,
        ..Default::default()
    };
    let mut vulkan = VulkanContext::new_simulated();
    let mut xr =
        unsafe { OpenXrContext::try_new_android(&config, &mut vulkan, activity.as_raw().cast()) }
            .map_err(|e| e.to_string())?;
    if passthrough && !xr.is_passthrough_enabled() {
        return Err("Passthrough is unavailable on this headset".into());
    }
    let mut controls = crate::controls::PlaybackControls::default();
    let mut surface_started = false;
    let screen = VirtualScreenConfig::default();
    let render_result = (|| -> Result<(), String> {
        while EPOCH.load(Ordering::Acquire) == epoch && !xr.should_exit() {
            xr.poll_events().map_err(|e| e.to_string())?;
            if !xr.is_session_running() {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            if !xr.swapchains_created() {
                xr.create_swapchains(&config).map_err(|e| e.to_string())?;
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
            if !frame.should_render {
                xr.end_frame_empty(frame.predicted_display_time)
                    .map_err(|e| e.to_string())?;
                continue;
            }
            let space = xr.stage_space().ok_or("No OpenXR stage space")?;
            let swapchain = xr.desktop_swapchain().ok_or("No OpenXR video swapchain")?;
            let layers =
                build_stereo_quad_layers(space, swapchain, format, order, width, height, &screen)
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
