//! Vulkan renderer core (ash, dynamically loaded libvulkan.so.1 — works on
//! Linux/FreeBSD/MidnightBSD).
//!
//! Frame graph per frame:
//!   scene quads+text -> lowres offscreen (renderScale * window)
//!   [FSR on]  EASU compute: lowres -> hiresA, RCAS compute: hiresA -> hiresB
//!   present:  fullscreen triangle sampling (hiresB | lowres) -> swapchain

use ash::vk;
use ash::{khr, Device, Entry, Instance};

use super::draw;
use super::fsr;
use super::shaders;

include!("../../../assets/font8x8.rs");

const OFFSCREEN_FORMAT_HI: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
const OFFSCREEN_FORMAT_LO: vk::Format = vk::Format::R8G8B8A8_UNORM;

pub fn mem_type(
    props: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    required: vk::MemoryPropertyFlags,
) -> Option<u32> {
    (0..props.memory_type_count)
        .find(|&i| {
            type_bits & (1 << i) != 0 && props.memory_types[i as usize].property_flags.contains(required)
        })
        .map(|i| i as u32)
}


pub struct Core {
    pub entry: Entry,
    pub instance: Instance,
    pub surface: vk::SurfaceKHR,
    pub surf_ext: khr::surface::Instance,
    pub phys: vk::PhysicalDevice,
    pub device: Device,
    pub queue: vk::Queue,
    pub qfam: u32,
    pub swap_dev: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub sc_format: vk::Format,
    pub extent: vk::Extent2D,
    sc_images: Vec<vk::Image>,
    sc_views: Vec<vk::ImageView>,
    sc_fbs: Vec<vk::Framebuffer>,
    off_format: vk::Format,
    lowres: vk::Image,
    lowres_mem: vk::DeviceMemory,
    lowres_view: vk::ImageView,
    hires_a: vk::Image,
    hires_a_mem: vk::DeviceMemory,
    hires_a_view: vk::ImageView,
    hires_b: vk::Image,
    hires_b_mem: vk::DeviceMemory,
    hires_b_view: vk::ImageView,
    scene_fb: vk::Framebuffer,
    font_img: vk::Image,
    font_mem: vk::DeviceMemory,
    font_view: vk::ImageView,
    rp_scene: vk::RenderPass,
    rp_present: vk::RenderPass,
    pipe_scene: vk::Pipeline,
    pipe_present: vk::Pipeline,
    pipe_easu: vk::Pipeline,
    pipe_rcas: vk::Pipeline,
    layout_scene: vk::PipelineLayout,
    layout_present: vk::PipelineLayout,
    layout_easu: vk::PipelineLayout,
    layout_rcas: vk::PipelineLayout,
    dpool: vk::DescriptorPool,
    dsl_scene: vk::DescriptorSetLayout,
    dsl_present: vk::DescriptorSetLayout,
    dsl_easu: vk::DescriptorSetLayout,
    dsl_rcas: vk::DescriptorSetLayout,
    set_scene: vk::DescriptorSet,
    set_present_low: vk::DescriptorSet,
    set_present_hi: vk::DescriptorSet,
    set_easu: vk::DescriptorSet,
    set_rcas: vk::DescriptorSet,
    vbuf: vk::Buffer,
    vbuf_mem: vk::DeviceMemory,
    vbuf_ptr: *mut u8,
    vbuf_size: usize,
    ubo_scene: vk::Buffer,
    ubo_scene_mem: vk::DeviceMemory,
    ubo_scene_ptr: *mut u8,
    ubo_easu: vk::Buffer,
    ubo_easu_mem: vk::DeviceMemory,
    ubo_easu_ptr: *mut u8,
    ubo_rcas: vk::Buffer,
    ubo_rcas_mem: vk::DeviceMemory,
    ubo_rcas_ptr: *mut u8,
    shot_buf: vk::Buffer,
    shot_mem: vk::DeviceMemory,
    shot_ptr: *mut u8,
    pool: vk::CommandPool,
    pub cmdbuf: vk::CommandBuffer,
    pub fence: vk::Fence,
    sem_img: vk::Semaphore,
    sem_done: vk::Semaphore,
    pub render_scale: f32,
    pub lw: u32,
    pub lh: u32,
}

impl Core {
    pub fn base_instance(ext_names: &[*const std::ffi::c_char]) -> Result<(Entry, Instance), String> {
        let entry = unsafe { Entry::load() }.map_err(|e| format!("load libvulkan: {}", e))?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"Wall Street Champion")
            .engine_name(c"wsc")
            .api_version(vk::API_VERSION_1_2);
        let ci = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(ext_names);
        let instance = unsafe { entry.create_instance(&ci, None) }.map_err(|e| format!("instance: {:?}", e))?;
        Ok((entry, instance))
    }

    fn pick_device(instance: &Instance, surface: vk::SurfaceKHR, surf: &khr::surface::Instance) -> Result<(vk::PhysicalDevice, u32, Device), String> {
        let physs = unsafe { instance.enumerate_physical_devices() }.map_err(|e| format!("phys devs: {:?}", e))?;
        let mut best: Option<(vk::PhysicalDevice, u32, u32)> = None;
        for p in physs {
            let props = unsafe { instance.get_physical_device_properties(p) };
            let qfams = unsafe { instance.get_physical_device_queue_family_properties(p) };
            for (i, q) in qfams.iter().enumerate() {
                if q.queue_flags.contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                    && unsafe { surf.get_physical_device_surface_support(p, i as u32, surface) }.unwrap_or(false)
                {
                    let rank = match props.device_type {
                        vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                        vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                        vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
                        _ => 0,
                    };
                    if best.map(|(_, _, r)| rank > r).unwrap_or(true) {
                        best = Some((p, i as u32, rank));
                    }
                }
            }
        }
        let (phys, qfam, _) = best.ok_or("no Vulkan device with graphics+compute+surface support")?;
        let dev_ext = [khr::swapchain::NAME.as_ptr()];
        let queue_ci = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(qfam)
            .queue_priorities(&[1.0]);
        let ci = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_ci))
            .enabled_extension_names(&dev_ext);
        let device = unsafe { instance.create_device(phys, &ci, None) }.map_err(|e| format!("device: {:?}", e))?;
        Ok((phys, qfam, device))
    }

    pub fn new_common(
        entry: Entry,
        instance: Instance,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
        render_scale: f32,
    ) -> Result<Core, String> {
        let surf_ext = khr::surface::Instance::new(&entry, &instance);
        let (phys, qfam, device) = Self::pick_device(&instance, surface, &surf_ext)?;
        let queue = unsafe { device.get_device_queue(qfam, 0) };

        // Offscreen format: prefer fp16 (better FSR quality), fall back to RGBA8.
        let fp = unsafe { instance.get_physical_device_format_properties(phys, OFFSCREEN_FORMAT_HI) };
        let off_format = if fp.optimal_tiling_features.contains(
            vk::FormatFeatureFlags::SAMPLED_IMAGE | vk::FormatFeatureFlags::STORAGE_IMAGE | vk::FormatFeatureFlags::COLOR_ATTACHMENT,
        ) {
            OFFSCREEN_FORMAT_HI
        } else {
            OFFSCREEN_FORMAT_LO
        };

        let caps = unsafe { surf_ext.get_physical_device_surface_capabilities(phys, surface) }.map_err(|e| format!("caps: {:?}", e))?;
        let formats = unsafe { surf_ext.get_physical_device_surface_formats(phys, surface) }.map_err(|e| format!("formats: {:?}", e))?;
        let sc_format = formats
            .iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
            .or_else(|| formats.first())
            .ok_or("no surface formats")?
            .format;
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: height.clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        let swap_dev = khr::swapchain::Device::new(&instance, &device);
        let image_count = (caps.min_image_count + 1).min(if caps.max_image_count == 0 { caps.min_image_count + 1 } else { caps.max_image_count });
        let sc_ci = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(sc_format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(caps.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);
        let swapchain = unsafe { swap_dev.create_swapchain(&sc_ci, None) }.map_err(|e| format!("swapchain: {:?}", e))?;

        let pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(qfam).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .map_err(|e| format!("pool: {:?}", e))?;
        let cmdbuf = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|e| format!("cmdbuf: {:?}", e))?[0];
        let fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None) }.map_err(|e| format!("fence: {:?}", e))?;
        let sem_img = unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }.map_err(|e| format!("sem: {:?}", e))?;
        let sem_done = unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }.map_err(|e| format!("sem: {:?}", e))?;

        // Shaders (WGSL -> SPIR-V via naga at runtime).
        let mk_module = |src: &str, name: &str| -> Result<vk::ShaderModule, String> {
            let words = shaders::wgsl_to_spv(src, name)?;
            unsafe { device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None) }
                .map_err(|e| format!("shader {}: {:?}", name, e))
        };
        let mod_scene = mk_module(shaders::SCENE_WGSL, "scene")?;
        let mod_present = mk_module(shaders::PRESENT_WGSL, "present")?;
        let mod_easu = mk_module(shaders::EASU_WGSL, "easu")?;
        let mod_rcas = mk_module(shaders::RCAS_WGSL, "rcas")?;

        let mk_dsl = |bindings: &[vk::DescriptorSetLayoutBinding]| -> Result<vk::DescriptorSetLayout, String> {
            unsafe {
                device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(bindings),
                    None,
                )
            }
            .map_err(|e| format!("dsl: {:?}", e))
        };
        let ubo = |binding: u32, stage: vk::ShaderStageFlags| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(stage)
        };
        let sampled = |binding: u32, stage: vk::ShaderStageFlags| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(stage)
        };
        let storage = |binding: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
        };
        let dsl_scene = mk_dsl(&[ubo(0, vk::ShaderStageFlags::VERTEX), sampled(1, vk::ShaderStageFlags::FRAGMENT)])?;
        let dsl_present = mk_dsl(&[sampled(0, vk::ShaderStageFlags::FRAGMENT)])?;
        let dsl_easu = mk_dsl(&[ubo(0, vk::ShaderStageFlags::COMPUTE), sampled(1, vk::ShaderStageFlags::COMPUTE), storage(2)])?;
        let dsl_rcas = mk_dsl(&[ubo(0, vk::ShaderStageFlags::COMPUTE), sampled(1, vk::ShaderStageFlags::COMPUTE), storage(2)])?;

        let dpool = unsafe {
            device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(8)
                    .pool_sizes(&[
                        vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(4),
                        vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(6),
                        vk::DescriptorPoolSize::default().ty(vk::DescriptorType::STORAGE_IMAGE).descriptor_count(2),
                    ]),
                None,
            )
        }
        .map_err(|e| format!("dpool: {:?}", e))?;
        let layouts = [dsl_scene, dsl_present, dsl_present, dsl_easu, dsl_rcas];
        let sets = unsafe { device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(dpool).set_layouts(&layouts)) }
            .map_err(|e| format!("dsets: {:?}", e))?;
        let (set_scene, set_present_low, set_present_hi, set_easu, set_rcas) =
            (sets[0], sets[1], sets[2], sets[3], sets[4]);

        let mem_props = unsafe { instance.get_physical_device_memory_properties(phys) };
        let mk_buffer = |size: vk::DeviceSize, usage: vk::BufferUsageFlags, flags: vk::MemoryPropertyFlags| -> Result<(vk::Buffer, vk::DeviceMemory, *mut u8), String> {
            let buf = unsafe { device.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(usage), None) }.map_err(|e| format!("buf: {:?}", e))?;
            let req = unsafe { device.get_buffer_memory_requirements(buf) };
            let ti = mem_type(&mem_props, req.memory_type_bits, flags).ok_or("no mem type for buffer")?;
            let mem = unsafe { device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ti), None) }.map_err(|e| format!("mem: {:?}", e))?;
            unsafe { device.bind_buffer_memory(buf, mem, 0) }.map_err(|e| format!("bind: {:?}", e))?;
            let ptr = if flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
                unsafe { device.map_memory(mem, 0, size, vk::MemoryMapFlags::empty()) }.map_err(|e| format!("map: {:?}", e))? as *mut u8
            } else {
                std::ptr::null_mut()
            };
            Ok((buf, mem, ptr))
        };
        let vbuf_size = 16 * 1024 * 1024;
        let (vbuf, vbuf_mem, vbuf_ptr) = mk_buffer(vbuf_size as u64, vk::BufferUsageFlags::VERTEX_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;
        let (ubo_scene, ubo_scene_mem, ubo_scene_ptr) = mk_buffer(256, vk::BufferUsageFlags::UNIFORM_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;
        let (ubo_easu, ubo_easu_mem, ubo_easu_ptr) = mk_buffer(256, vk::BufferUsageFlags::UNIFORM_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;
        let (ubo_rcas, ubo_rcas_mem, ubo_rcas_ptr) = mk_buffer(256, vk::BufferUsageFlags::UNIFORM_BUFFER, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;
        let shot_size = extent.width as u64 * extent.height as u64 * 4;
        let (shot_buf, shot_mem, shot_ptr) = mk_buffer(shot_size, vk::BufferUsageFlags::TRANSFER_DST, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT)?;

        let mk_image = |w: u32, h: u32, format: vk::Format, usage: vk::ImageUsageFlags| -> Result<(vk::Image, vk::DeviceMemory), String> {
            let img = unsafe {
                device.create_image(
                    &vk::ImageCreateInfo::default()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D { width: w, height: h, depth: 1 })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(usage)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
            }
            .map_err(|e| format!("image: {:?}", e))?;
            let req = unsafe { device.get_image_memory_requirements(img) };
            let ti = mem_type(&mem_props, req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL).ok_or("no mem type for image")?;
            let mem = unsafe { device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ti), None) }.map_err(|e| format!("mem: {:?}", e))?;
            unsafe { device.bind_image_memory(img, mem, 0) }.map_err(|e| format!("bind img: {:?}", e))?;
            Ok((img, mem))
        };
        let mk_view = |img: vk::Image, format: vk::Format| -> Result<vk::ImageView, String> {
            unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(img)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                )
            }
            .map_err(|e| format!("view: {:?}", e))
        };

        let lw = ((extent.width as f32 * render_scale).round() as u32).max(8);
        let lh = ((extent.height as f32 * render_scale).round() as u32).max(8);
        let (lowres, lowres_mem) = mk_image(lw, lh, off_format, vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)?;
        let lowres_view = mk_view(lowres, off_format)?;
        let (hires_a, hires_a_mem) = mk_image(extent.width, extent.height, off_format, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE)?;
        let hires_a_view = mk_view(hires_a, off_format)?;
        let (hires_b, hires_b_mem) = mk_image(extent.width, extent.height, off_format, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::STORAGE)?;
        let hires_b_view = mk_view(hires_b, off_format)?;
        let (font_img, font_mem) = mk_image(128, 128, vk::Format::R8_UNORM, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)?;
        let font_view = mk_view(font_img, vk::Format::R8_UNORM)?;

        // Render passes.
        let att_scene = vk::AttachmentDescription::default()
            .format(off_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let att_ref_scene = vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let subpass_scene = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&att_ref_scene));
        let dep_scene = vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);
        let rp_scene = unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&att_scene))
                    .subpasses(std::slice::from_ref(&subpass_scene))
                    .dependencies(std::slice::from_ref(&dep_scene)),
                None,
            )
        }
        .map_err(|e| format!("rp scene: {:?}", e))?;

        let att_present = vk::AttachmentDescription::default()
            .format(sc_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
        let att_ref_present = vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let subpass_present = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&att_ref_present));
        let rp_present = unsafe {
            device.create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(std::slice::from_ref(&att_present))
                    .subpasses(std::slice::from_ref(&subpass_present)),
                None,
            )
        }
        .map_err(|e| format!("rp present: {:?}", e))?;

        // Pipeline layouts.
        let mk_playout = |dsl: vk::DescriptorSetLayout| -> Result<vk::PipelineLayout, String> {
            unsafe { device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(std::slice::from_ref(&dsl)), None) }
                .map_err(|e| format!("playout: {:?}", e))
        };
        let layout_scene = mk_playout(dsl_scene)?;
        let layout_present = mk_playout(dsl_present)?;
        let layout_easu = mk_playout(dsl_easu)?;
        let layout_rcas = mk_playout(dsl_rcas)?;

        // Graphics pipelines.
        let binding = vk::VertexInputBindingDescription::default().binding(0).stride(40).input_rate(vk::VertexInputRate::VERTEX);
        let attrs = [
            vk::VertexInputAttributeDescription::default().location(0).binding(0).format(vk::Format::R32G32_SFLOAT).offset(0),
            vk::VertexInputAttributeDescription::default().location(1).binding(0).format(vk::Format::R32G32B32A32_SFLOAT).offset(8),
            vk::VertexInputAttributeDescription::default().location(2).binding(0).format(vk::Format::R32G32_SFLOAT).offset(24),
            vk::VertexInputAttributeDescription::default().location(3).binding(0).format(vk::Format::R32_SFLOAT).offset(32),
        ];
        let vi = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attrs);
        let vi_empty = vk::PipelineVertexInputStateCreateInfo::default();
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rs = vk::PipelineRasterizationStateCreateInfo::default()
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend_att = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);
        let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(std::slice::from_ref(&blend_att));
        let dyn_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dyns = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyn_states);
        let dss = vk::PipelineDepthStencilStateCreateInfo::default();
        let stage = |stage: vk::ShaderStageFlags, module: vk::ShaderModule, name: &'static std::ffi::CStr| {
            vk::PipelineShaderStageCreateInfo::default().stage(stage).module(module).name(name)
        };
        let mk_graphics = |vs: vk::ShaderModule, fs: vk::ShaderModule, vi_state: &vk::PipelineVertexInputStateCreateInfo, layout: vk::PipelineLayout, rp: vk::RenderPass, name: &str| -> Result<vk::Pipeline, String> {
            unsafe {
                device.create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[vk::GraphicsPipelineCreateInfo::default()
                        .stages(&[stage(vk::ShaderStageFlags::VERTEX, vs, c"vs"), stage(vk::ShaderStageFlags::FRAGMENT, fs, c"fs")])
                        .vertex_input_state(vi_state)
                        .input_assembly_state(&ia)
                        .viewport_state(&vp)
                        .rasterization_state(&rs)
                        .multisample_state(&ms)
                        .color_blend_state(&cb)
                        .dynamic_state(&dyns)
                        .depth_stencil_state(&dss)
                        .layout(layout)
                        .render_pass(rp)
                        .subpass(0)],
                    None,
                )
            }
            .map_err(|(e, _)| format!("pipe {}: {:?}", name, e))
            .map(|v| v[0])
        };
        let pipe_scene = mk_graphics(mod_scene, mod_scene, &vi, layout_scene, rp_scene, "scene")?;
        let pipe_present = mk_graphics(mod_present, mod_present, &vi_empty, layout_present, rp_present, "present")?;
        let mk_compute = |module: vk::ShaderModule, layout: vk::PipelineLayout, name: &str| -> Result<vk::Pipeline, String> {
            unsafe {
                device.create_compute_pipelines(
                    vk::PipelineCache::null(),
                    &[vk::ComputePipelineCreateInfo::default().stage(stage(vk::ShaderStageFlags::COMPUTE, module, c"main")).layout(layout)],
                    None,
                )
            }
            .map_err(|(e, _)| format!("pipe {}: {:?}", name, e))
            .map(|v| v[0])
        };
        let pipe_easu = mk_compute(mod_easu, layout_easu, "easu")?;
        let pipe_rcas = mk_compute(mod_rcas, layout_rcas, "rcas")?;

        let scene_fb = unsafe {
            device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(rp_scene)
                    .attachments(std::slice::from_ref(&lowres_view))
                    .width(lw)
                    .height(lh)
                    .layers(1),
                None,
            )
        }
        .map_err(|e| format!("fb scene: {:?}", e))?;

        let mut core = Core {
            entry,
            instance,
            surface,
            surf_ext,
            phys,
            device,
            queue,
            qfam,
            swap_dev,
            swapchain,
            sc_format,
            extent,
            sc_images: Vec::new(),
            sc_views: Vec::new(),
            sc_fbs: Vec::new(),
            off_format,
            lowres,
            lowres_mem,
            lowres_view,
            hires_a,
            hires_a_mem,
            hires_a_view,
            hires_b,
            hires_b_mem,
            hires_b_view,
            scene_fb,
            font_img,
            font_mem,
            font_view,
            rp_scene,
            rp_present,
            pipe_scene,
            pipe_present,
            pipe_easu,
            pipe_rcas,
            layout_scene,
            layout_present,
            layout_easu,
            layout_rcas,
            dpool,
            dsl_scene,
            dsl_present,
            dsl_easu,
            dsl_rcas,
            set_scene,
            set_present_low,
            set_present_hi,
            set_easu,
            set_rcas,
            vbuf,
            vbuf_mem,
            vbuf_ptr,
            vbuf_size,
            ubo_scene,
            ubo_scene_mem,
            ubo_scene_ptr,
            ubo_easu,
            ubo_easu_mem,
            ubo_easu_ptr,
            ubo_rcas,
            ubo_rcas_mem,
            ubo_rcas_ptr,
            shot_buf,
            shot_mem,
            shot_ptr,
            pool,
            cmdbuf,
            fence,
            sem_img,
            sem_done,
            render_scale,
            lw,
            lh,
        };
        core.recreate_swapchain_images()?;
        core.upload_font()?;
        core.one_time_layouts()?;
        core.write_descriptors()?;
        core.write_rcas_consts()?;
        core.transition_lowres_to_read()?;
        Ok(core)
    }

    pub fn recreate_swapchain_images(&mut self) -> Result<(), String> {
        unsafe {
            self.device.device_wait_idle().map_err(|e| format!("idle: {:?}", e))?;
            for fb in &self.sc_fbs {
                self.device.destroy_framebuffer(*fb, None);
            }
            for v in &self.sc_views {
                self.device.destroy_image_view(*v, None);
            }
            self.sc_fbs.clear();
            self.sc_views.clear();
            self.sc_images = self.swap_dev.get_swapchain_images(self.swapchain).map_err(|e| format!("sc imgs: {:?}", e))?;
            for &img in &self.sc_images {
                let view = self.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(img)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.sc_format)
                        .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                    None,
                ).map_err(|e| format!("sc view: {:?}", e))?;
                let fb = self.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.rp_present)
                        .attachments(std::slice::from_ref(&view))
                        .width(self.extent.width)
                        .height(self.extent.height)
                        .layers(1),
                    None,
                ).map_err(|e| format!("sc fb: {:?}", e))?;
                self.sc_views.push(view);
                self.sc_fbs.push(fb);
            }
        }
        Ok(())
    }

    /// Recreate the low-res offscreen (render scale change / FSR toggle).
    pub fn set_render_scale(&mut self, scale: f32) -> Result<(), String> {
        self.render_scale = scale;
        self.lw = ((self.extent.width as f32 * scale).round() as u32).max(8);
        self.lh = ((self.extent.height as f32 * scale).round() as u32).max(8);
        unsafe {
            self.device.device_wait_idle().map_err(|e| format!("idle: {:?}", e))?;
            self.device.destroy_framebuffer(self.scene_fb, None);
            self.device.destroy_image_view(self.lowres_view, None);
            self.device.destroy_image(self.lowres, None);
            self.device.free_memory(self.lowres_mem, None);
            let mem_props = self.instance.get_physical_device_memory_properties(self.phys);
            let img = self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(self.off_format)
                    .extent(vk::Extent3D { width: self.lw, height: self.lh, depth: 1 })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            ).map_err(|e| format!("lowres: {:?}", e))?;
            let req = self.device.get_image_memory_requirements(img);
            let ti = mem_type(&mem_props, req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL).ok_or("no mem type")?;
            let mem = self.device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ti), None).map_err(|e| format!("mem: {:?}", e))?;
            self.device.bind_image_memory(img, mem, 0).map_err(|e| format!("bind: {:?}", e))?;
            let view = self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(img)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.off_format)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1)),
                None,
            ).map_err(|e| format!("view: {:?}", e))?;
            self.scene_fb = self.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(self.rp_scene)
                    .attachments(std::slice::from_ref(&view))
                    .width(self.lw)
                    .height(self.lh)
                    .layers(1),
                None,
            ).map_err(|e| format!("fb: {:?}", e))?;
            self.lowres = img;
            self.lowres_mem = mem;
            self.lowres_view = view;
        }
        self.transition_lowres_to_read()?;
        self.write_descriptors()?;
        Ok(())
    }

    fn submit_one_shot(&self, record: impl FnOnce()) -> Result<(), String> {
        unsafe {
            self.device.reset_command_buffer(self.cmdbuf, vk::CommandBufferResetFlags::empty()).map_err(|e| format!("reset: {:?}", e))?;
            self.device.begin_command_buffer(self.cmdbuf, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e| format!("begin: {:?}", e))?;
            record();
            self.device.end_command_buffer(self.cmdbuf).map_err(|e| format!("end: {:?}", e))?;
            let si = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&self.cmdbuf));
            self.device.queue_submit(self.queue, &[si], self.fence).map_err(|e| format!("submit: {:?}", e))?;
            self.device.wait_for_fences(&[self.fence], true, u64::MAX).map_err(|e| format!("wait: {:?}", e))?;
            // Leave the fence SIGNALED: render() waits on it at the start of the
            // next frame before re-recording the command buffer.
        }
        Ok(())
    }

    /// Block until all GPU work is done (used before reading screenshots).
    pub fn wait_idle(&self) -> Result<(), String> {
        unsafe { self.device.device_wait_idle().map_err(|e| format!("idle: {:?}", e)) }
    }

    fn transition_lowres_to_read(&self) -> Result<(), String> {
        unsafe {
            let b = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(self.lowres)
                .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);
            self.submit_one_shot(|| {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[b]);
            })
        }
    }

    fn upload_font(&self) -> Result<(), String> {
        let size = 128u64 * 128;
        unsafe {
            let buf = self.device.create_buffer(&vk::BufferCreateInfo::default().size(size).usage(vk::BufferUsageFlags::TRANSFER_SRC), None).map_err(|e| format!("font buf: {:?}", e))?;
            let mem_props = self.instance.get_physical_device_memory_properties(self.phys);
            let req = self.device.get_buffer_memory_requirements(buf);
            let ti = mem_type(&mem_props, req.memory_type_bits, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT).ok_or("no host mem")?;
            let mem = self.device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ti), None).map_err(|e| format!("mem: {:?}", e))?;
            self.device.bind_buffer_memory(buf, mem, 0).map_err(|e| format!("bind: {:?}", e))?;
            let ptr = self.device.map_memory(mem, 0, size, vk::MemoryMapFlags::empty()).map_err(|e| format!("map: {:?}", e))? as *mut u8;
            let mut data = vec![0u8; size as usize];
            for (code, rows) in FONT8X8.iter().enumerate() {
                for (ry, &bits) in rows.iter().enumerate() {
                    for bit in 0..8 {
                        if bits & (0x80 >> bit) != 0 {
                            // font8x8 stores the leftmost pixel in the LSB of the row byte
                            data[(code / 16) * 8 * 128 + (code % 16) * 8 + ry * 128 + (7 - bit)] = 255;
                        }
                    }
                }
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            self.device.unmap_memory(mem);
            let b1 = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(self.font_img)
                .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1));
            let b2 = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(self.font_img)
                .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);
            self.submit_one_shot(|| {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[b1]);
                self.device.cmd_copy_buffer_to_image(
                    self.cmdbuf,
                    buf,
                    self.font_img,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[vk::BufferImageCopy::default()
                        .buffer_offset(0)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                        .image_extent(vk::Extent3D { width: 128, height: 128, depth: 1 })],
                );
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[b2]);
            })?;
            self.device.destroy_buffer(buf, None);
            self.device.free_memory(mem, None);
        }
        Ok(())
    }

    fn one_time_layouts(&self) -> Result<(), String> {
        unsafe {
            let mk = |img: vk::Image| {
                vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::GENERAL)
                    .image(img)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))
                    .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)
            };
            self.submit_one_shot(|| {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::COMPUTE_SHADER, vk::DependencyFlags::empty(), &[], &[], &[mk(self.hires_a), mk(self.hires_b)]);
            })
        }
    }

    fn write_descriptors(&self) -> Result<(), String> {
        unsafe {
            let ubo_scene_info = vk::DescriptorBufferInfo::default().buffer(self.ubo_scene).range(256);
            let ubo_easu_info = vk::DescriptorBufferInfo::default().buffer(self.ubo_easu).range(256);
            let ubo_rcas_info = vk::DescriptorBufferInfo::default().buffer(self.ubo_rcas).range(256);
            let img = |view: vk::ImageView| {
                vk::DescriptorImageInfo::default().image_view(view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            };
            let img_gen = |view: vk::ImageView| {
                vk::DescriptorImageInfo::default().image_view(view).image_layout(vk::ImageLayout::GENERAL)
            };
            let font_ii = img(self.font_view);
            let low_ii = img(self.lowres_view);
            let hb_ii = img_gen(self.hires_b_view);
            let ha_ii = img_gen(self.hires_a_view);
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(self.set_scene).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(std::slice::from_ref(&ubo_scene_info)),
                vk::WriteDescriptorSet::default().dst_set(self.set_scene).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(std::slice::from_ref(&font_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_present_low).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(std::slice::from_ref(&low_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_present_hi).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(std::slice::from_ref(&hb_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_easu).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(std::slice::from_ref(&ubo_easu_info)),
                vk::WriteDescriptorSet::default().dst_set(self.set_easu).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(std::slice::from_ref(&low_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_easu).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_IMAGE).image_info(std::slice::from_ref(&ha_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_rcas).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(std::slice::from_ref(&ubo_rcas_info)),
                vk::WriteDescriptorSet::default().dst_set(self.set_rcas).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(std::slice::from_ref(&ha_ii)),
                vk::WriteDescriptorSet::default().dst_set(self.set_rcas).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_IMAGE).image_info(std::slice::from_ref(&hb_ii)),
            ];
            self.device.update_descriptor_sets(&writes, &[]);
        }
        Ok(())
    }

    fn write_rcas_consts(&self) -> Result<(), String> {
        let sharp = fsr::rcas_sharpness(fsr::RCAS_STOPS);
        let data = [sharp, 0.0f32, 0.0, 0.0];
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, self.ubo_rcas_ptr, 16);
        }
        Ok(())
    }

    fn write_easu_consts(&self) {
        let c = fsr::easu_consts(self.lw as f32, self.lh as f32, self.extent.width as f32, self.extent.height as f32);
        unsafe {
            std::ptr::copy_nonoverlapping(c.as_ptr() as *const u8, self.ubo_easu_ptr, 64);
        }
    }

    pub fn device_name(&self) -> String {
        unsafe {
            self.instance
                .get_physical_device_properties(self.phys)
                .device_name_as_c_str()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        }
    }

    /// Render one frame. If `shot` is true, the swapchain image is copied to
    /// the shot buffer; read it with `take_shot()` afterwards.
    pub fn render(&mut self, batch: &draw::Batch, fsr_on: bool, shot: bool) -> Result<(), String> {
        let bytes = batch.v.len() * 4;
        if bytes > self.vbuf_size {
            return Err("vertex buffer overflow".to_string());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(batch.v.as_ptr() as *const u8, self.vbuf_ptr, bytes);
            let scene_u = [self.extent.width as f32, self.extent.height as f32, 0.0f32, 0.0];
            std::ptr::copy_nonoverlapping(scene_u.as_ptr() as *const u8, self.ubo_scene_ptr, 16);
        }
        self.write_easu_consts();

        unsafe {
            let (idx, _suboptimal) = match self.swap_dev.acquire_next_image(self.swapchain, u64::MAX, self.sem_img, vk::Fence::null()) {
                Ok(r) => r,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Err("out_of_date".to_string()),
                Err(e) => return Err(format!("acquire: {:?}", e)),
            };
            // Fence is signaled => previous frame finished; safe to re-record.
            self.device.wait_for_fences(&[self.fence], true, u64::MAX).map_err(|e| format!("wait fence: {:?}", e))?;
            self.device.reset_fences(&[self.fence]).map_err(|e| format!("reset fence: {:?}", e))?;
            self.device.reset_command_buffer(self.cmdbuf, vk::CommandBufferResetFlags::empty()).map_err(|e| format!("reset cb: {:?}", e))?;
            self.device.begin_command_buffer(self.cmdbuf, &vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e| format!("begin cb: {:?}", e))?;

            // --- scene pass into lowres ---
            let clear = vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.045, 0.055, 0.075, 1.0] },
            };
            self.device.cmd_begin_render_pass(
                self.cmdbuf,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.rp_scene)
                    .framebuffer(self.scene_fb)
                    .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: self.lw, height: self.lh } })
                    .clear_values(std::slice::from_ref(&clear)),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(self.cmdbuf, vk::PipelineBindPoint::GRAPHICS, self.pipe_scene);
            self.device.cmd_set_viewport(self.cmdbuf, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.lw as f32, height: self.lh as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmdbuf, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: vk::Extent2D { width: self.lw, height: self.lh } }]);
            self.device.cmd_bind_descriptor_sets(self.cmdbuf, vk::PipelineBindPoint::GRAPHICS, self.layout_scene, 0, &[self.set_scene], &[]);
            self.device.cmd_bind_vertex_buffers(self.cmdbuf, 0, &[self.vbuf], &[0]);
            self.device.cmd_draw(self.cmdbuf, (batch.v.len() / 10) as u32, 1, 0, 0);
            self.device.cmd_end_render_pass(self.cmdbuf);

            let barrier = |img: vk::Image, src_acc: vk::AccessFlags, dst_acc: vk::AccessFlags, old: vk::ImageLayout, new: vk::ImageLayout| {
                vk::ImageMemoryBarrier::default()
                    .old_layout(old)
                    .new_layout(new)
                    .image(img)
                    .subresource_range(vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1))
                    .src_access_mask(src_acc)
                    .dst_access_mask(dst_acc)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            };

            // --- FSR 1.x passes ---
            if fsr_on {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::PipelineStageFlags::COMPUTE_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.lowres, vk::AccessFlags::COLOR_ATTACHMENT_WRITE, vk::AccessFlags::SHADER_READ, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]);
                self.device.cmd_bind_pipeline(self.cmdbuf, vk::PipelineBindPoint::COMPUTE, self.pipe_easu);
                self.device.cmd_bind_descriptor_sets(self.cmdbuf, vk::PipelineBindPoint::COMPUTE, self.layout_easu, 0, &[self.set_easu], &[]);
                self.device.cmd_dispatch(self.cmdbuf, (self.extent.width + 7) / 8, (self.extent.height + 7) / 8, 1);
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::COMPUTE_SHADER, vk::PipelineStageFlags::COMPUTE_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.hires_a, vk::AccessFlags::SHADER_WRITE, vk::AccessFlags::SHADER_READ, vk::ImageLayout::GENERAL, vk::ImageLayout::GENERAL)]);
                self.device.cmd_bind_pipeline(self.cmdbuf, vk::PipelineBindPoint::COMPUTE, self.pipe_rcas);
                self.device.cmd_bind_descriptor_sets(self.cmdbuf, vk::PipelineBindPoint::COMPUTE, self.layout_rcas, 0, &[self.set_rcas], &[]);
                self.device.cmd_dispatch(self.cmdbuf, (self.extent.width + 7) / 8, (self.extent.height + 7) / 8, 1);
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::COMPUTE_SHADER, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.hires_b, vk::AccessFlags::SHADER_WRITE, vk::AccessFlags::SHADER_READ, vk::ImageLayout::GENERAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]);
            } else {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::PipelineStageFlags::FRAGMENT_SHADER, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.lowres, vk::AccessFlags::COLOR_ATTACHMENT_WRITE, vk::AccessFlags::SHADER_READ, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]);
            }

            // --- present pass to swapchain ---
            let clear2 = vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] },
            };
            self.device.cmd_begin_render_pass(
                self.cmdbuf,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.rp_present)
                    .framebuffer(self.sc_fbs[idx as usize])
                    .render_area(vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent })
                    .clear_values(std::slice::from_ref(&clear2)),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(self.cmdbuf, vk::PipelineBindPoint::GRAPHICS, self.pipe_present);
            self.device.cmd_set_viewport(self.cmdbuf, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: self.extent.width as f32, height: self.extent.height as f32, min_depth: 0.0, max_depth: 1.0 }]);
            self.device.cmd_set_scissor(self.cmdbuf, 0, &[vk::Rect2D { offset: vk::Offset2D { x: 0, y: 0 }, extent: self.extent }]);
            let present_set = if fsr_on { self.set_present_hi } else { self.set_present_low };
            self.device.cmd_bind_descriptor_sets(self.cmdbuf, vk::PipelineBindPoint::GRAPHICS, self.layout_present, 0, &[present_set], &[]);
            self.device.cmd_draw(self.cmdbuf, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(self.cmdbuf);

            // --- optional screenshot copy ---
            if shot {
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, vk::PipelineStageFlags::TRANSFER, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.sc_images[idx as usize], vk::AccessFlags::COLOR_ATTACHMENT_WRITE, vk::AccessFlags::TRANSFER_READ, vk::ImageLayout::PRESENT_SRC_KHR, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)]);
                self.device.cmd_copy_image_to_buffer(
                    self.cmdbuf,
                    self.sc_images[idx as usize],
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    self.shot_buf,
                    &[vk::BufferImageCopy::default()
                        .buffer_offset(0)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1))
                        .image_extent(vk::Extent3D { width: self.extent.width, height: self.extent.height, depth: 1 })],
                );
                self.device.cmd_pipeline_barrier(self.cmdbuf, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::BOTTOM_OF_PIPE, vk::DependencyFlags::empty(), &[], &[], &[barrier(self.sc_images[idx as usize], vk::AccessFlags::TRANSFER_READ, vk::AccessFlags::MEMORY_READ, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR)]);
            }

            self.device.end_command_buffer(self.cmdbuf).map_err(|e| format!("end cb: {:?}", e))?;
            let wait_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
            let si = vk::SubmitInfo::default()
                .wait_semaphores(std::slice::from_ref(&self.sem_img))
                .wait_dst_stage_mask(std::slice::from_ref(&wait_stage))
                .command_buffers(std::slice::from_ref(&self.cmdbuf))
                .signal_semaphores(std::slice::from_ref(&self.sem_done));
            self.device.queue_submit(self.queue, &[si], self.fence).map_err(|e| format!("submit: {:?}", e))?;
            let pi = vk::PresentInfoKHR::default()
                .wait_semaphores(std::slice::from_ref(&self.sem_done))
                .swapchains(std::slice::from_ref(&self.swapchain))
                .image_indices(std::slice::from_ref(&idx));
            let _ = self.swap_dev.queue_present(self.queue, &pi);
            // Do not wait here: the next render() call waits on the fence before
            // re-recording. The fence stays signaled after completion.
        }
        Ok(())
    }

    /// Read the screenshot buffer (BGRA) and return RGBA bytes.
    pub fn take_shot(&self) -> Vec<u8> {
        let n = (self.extent.width * self.extent.height * 4) as usize;
        let src = unsafe { std::slice::from_raw_parts(self.shot_ptr, n) };
        let mut rgba = vec![0u8; n];
        for i in 0..n / 4 {
            rgba[i * 4] = src[i * 4 + 2];
            rgba[i * 4 + 1] = src[i * 4 + 1];
            rgba[i * 4 + 2] = src[i * 4];
            rgba[i * 4 + 3] = 255;
        }
        rgba
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_semaphore(self.sem_img, None);
            self.device.destroy_semaphore(self.sem_done, None);
            self.device.destroy_command_pool(self.pool, None);
            for fb in &self.sc_fbs { self.device.destroy_framebuffer(*fb, None); }
            for v in &self.sc_views { self.device.destroy_image_view(*v, None); }
            self.swap_dev.destroy_swapchain(self.swapchain, None);
            self.device.destroy_framebuffer(self.scene_fb, None);
            self.device.destroy_image_view(self.lowres_view, None);
            self.device.destroy_image(self.lowres, None);
            self.device.free_memory(self.lowres_mem, None);
            self.device.destroy_image_view(self.hires_a_view, None);
            self.device.destroy_image(self.hires_a, None);
            self.device.free_memory(self.hires_a_mem, None);
            self.device.destroy_image_view(self.hires_b_view, None);
            self.device.destroy_image(self.hires_b, None);
            self.device.free_memory(self.hires_b_mem, None);
            self.device.destroy_image_view(self.font_view, None);
            self.device.destroy_image(self.font_img, None);
            self.device.free_memory(self.font_mem, None);
            self.device.destroy_pipeline(self.pipe_scene, None);
            self.device.destroy_pipeline(self.pipe_present, None);
            self.device.destroy_pipeline(self.pipe_easu, None);
            self.device.destroy_pipeline(self.pipe_rcas, None);
            self.device.destroy_pipeline_layout(self.layout_scene, None);
            self.device.destroy_pipeline_layout(self.layout_present, None);
            self.device.destroy_pipeline_layout(self.layout_easu, None);
            self.device.destroy_pipeline_layout(self.layout_rcas, None);
            self.device.destroy_render_pass(self.rp_scene, None);
            self.device.destroy_render_pass(self.rp_present, None);
            self.device.destroy_descriptor_pool(self.dpool, None);
            self.device.destroy_descriptor_set_layout(self.dsl_scene, None);
            self.device.destroy_descriptor_set_layout(self.dsl_present, None);
            self.device.destroy_descriptor_set_layout(self.dsl_easu, None);
            self.device.destroy_descriptor_set_layout(self.dsl_rcas, None);
            self.device.destroy_buffer(self.vbuf, None);
            self.device.free_memory(self.vbuf_mem, None);
            self.device.destroy_buffer(self.ubo_scene, None);
            self.device.free_memory(self.ubo_scene_mem, None);
            self.device.destroy_buffer(self.ubo_easu, None);
            self.device.free_memory(self.ubo_easu_mem, None);
            self.device.destroy_buffer(self.ubo_rcas, None);
            self.device.free_memory(self.ubo_rcas_mem, None);
            self.device.destroy_buffer(self.shot_buf, None);
            self.device.free_memory(self.shot_mem, None);
            self.device.destroy_device(None);
            self.surf_ext.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}
