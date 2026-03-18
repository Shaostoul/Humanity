//! Render pipeline management — creates and caches wgpu render pipelines.
//!
//! Pipeline definitions loaded from `config/pipelines.ron`.

/// Manages creation and caching of render pipelines.
pub struct PipelineManager {
    // TODO: HashMap<PipelineId, wgpu::RenderPipeline>
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {}
    }
}
