//! Exclusive access to the context's regular HUD swapchain during custom stereo submission.
//! Never acquire the Android video surface swapchain: its producer is MediaPlayer.
use ash::vk::{self, Handle};
use openxr::{Swapchain, Vulkan};
use xr_core::spatial::ScreenPose;
use xr_core::spatial::ScreenTransform;
use xr_video::graphics::vulkan::VulkanContext;

pub fn upload(sc: &Swapchain<Vulkan>, vulkan: &VulkanContext, rgba: &[u8]) -> Result<(), String> {
    upload_sized(sc, vulkan, rgba, 512, 256)
}
pub fn upload_sized(
    sc: &Swapchain<Vulkan>,
    vulkan: &VulkanContext,
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err("Invalid cinema atlas size".into());
    }
    let images = sc.enumerate_images().map_err(|e| e.to_string())?;
    let fp = sc.instance().fp();
    let mut index = 0;
    // The OpenXrContext exposes this swapchain by shared reference. The XR thread is its
    // sole user here; no context renderer is called until this acquire/wait/release completes.
    // Use raw functions rather than forging a mutable alias or a second owning Swapchain.
    unsafe {
        check((fp.acquire_swapchain_image)(
            sc.as_raw(),
            std::ptr::null(),
            &mut index,
        ))?;
        let wait = openxr::sys::SwapchainImageWaitInfo {
            ty: openxr::sys::StructureType::SWAPCHAIN_IMAGE_WAIT_INFO,
            next: std::ptr::null(),
            timeout: openxr::Duration::INFINITE,
        };
        check((fp.wait_swapchain_image)(sc.as_raw(), &wait))?;
        let uploaded = images
            .get(index as usize)
            .ok_or_else(|| "Invalid HUD image index".into())
            .and_then(|raw| {
                vulkan
                    .upload_rgba_to_image(vk::Image::from_raw(*raw), width, height, rgba)
                    .map_err(|e| e.to_string())
            });
        let released = check((fp.release_swapchain_image)(sc.as_raw(), std::ptr::null()));
        uploaded.and(released)
    }
}
fn check(result: openxr::sys::Result) -> Result<(), String> {
    if result.into_raw() < 0 {
        Err(format!("Cinema texture: {result:?}"))
    } else {
        Ok(())
    }
}
pub fn cylinder<'a>(
    space: &'a openxr::Space,
    sc: &'a Swapchain<Vulkan>,
    rect: (i32, i32, i32, i32),
    panel: &ScreenTransform,
) -> openxr::CompositionLayerCylinderKHR<'a, Vulkan> {
    openxr::CompositionLayerCylinderKHR::new()
        .space(space)
        .layer_flags(openxr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
        .eye_visibility(openxr::EyeVisibility::BOTH)
        .sub_image(
            openxr::SwapchainSubImage::new()
                .swapchain(sc)
                .image_rect(openxr::Rect2Di {
                    offset: openxr::Offset2Di {
                        x: rect.0,
                        y: rect.1,
                    },
                    extent: openxr::Extent2Di {
                        width: rect.2,
                        height: rect.3,
                    },
                }),
        )
        .pose(panel.cylinder_axis_pose().into())
        .radius(panel.curve_radius)
        .central_angle(panel.curve_angle)
        .aspect_ratio(panel.aspect_ratio)
}
pub fn quad<'a>(
    space: &'a openxr::Space,
    sc: &'a Swapchain<Vulkan>,
    rect: (i32, i32, i32, i32),
    pose: ScreenPose,
    size: (f32, f32),
) -> openxr::CompositionLayerQuad<'a, Vulkan> {
    openxr::CompositionLayerQuad::new()
        .space(space)
        .layer_flags(openxr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
        .eye_visibility(openxr::EyeVisibility::BOTH)
        .sub_image(
            openxr::SwapchainSubImage::new()
                .swapchain(sc)
                .image_rect(openxr::Rect2Di {
                    offset: openxr::Offset2Di {
                        x: rect.0,
                        y: rect.1,
                    },
                    extent: openxr::Extent2Di {
                        width: rect.2,
                        height: rect.3,
                    },
                }),
        )
        .pose(pose.into())
        .size(openxr::Extent2Df {
            width: size.0,
            height: size.1,
        })
}
/// Downsample the shared assets into reserved atlas rectangles, retaining the same DeskQuest art.
pub fn add_handles(rgba: &mut [u8]) {
    blit(
        rgba,
        &xr_core::openxr::overlay::cursor_rgba(),
        (96, 96),
        (0, 192, 48, 48),
    );
    blit(
        rgba,
        &xr_core::openxr::overlay::corner_rgba(1.0),
        (128, 128),
        (64, 192, 64, 64),
    );
    blit(
        rgba,
        &xr_core::openxr::overlay::bar_rgba(1.0),
        (384, 32),
        (128, 192, 128, 12),
    );
}
fn blit(out: &mut [u8], src: &[u8], source: (usize, usize), dest: (usize, usize, usize, usize)) {
    for y in 0..dest.3 {
        for x in 0..dest.2 {
            let from = ((y * source.1 / dest.3) * source.0 + x * source.0 / dest.2) * 4;
            let to = ((dest.1 + y) * 512 + dest.0 + x) * 4;
            out[to..to + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
}
